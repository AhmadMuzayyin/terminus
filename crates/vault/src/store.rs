use crate::crypto;
use crate::VaultError;
use terminus_core::{AuthMethod, ConnectionKind, HostGroup, HostProfile, Identity};
use rusqlite::{params, Connection, OptionalExtension, Row};
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Nilai konstan yang dienkripsi pakai key hasil derive dari master
/// password, disimpan sebagai "verifier" di `vault_meta`. Waktu unlock,
/// kalau decrypt sukses DAN hasilnya persis ini, master password-nya
/// benar — trik umum ("canary value") supaya tidak perlu mekanisme
/// verifikasi password terpisah dari cipher yang sudah dipakai.
const VAULT_VERIFIER_MARKER: &[u8] = b"terminus-vault-v1";

/// Handle ke database vault. Path default ada di direktori config OS
/// (lewat crate `directories`), misal:
/// - Linux: `~/.config/terminus/vault.db`
/// - macOS: `~/Library/Application Support/terminus/vault.db`
pub struct VaultStore {
    conn: Connection,
    db_path: PathBuf,
    unlocked_key: Option<[u8; 32]>,
}

impl VaultStore {
    pub fn open_default() -> Result<Self, VaultError> {
        let dirs = directories::ProjectDirs::from("com", "terminus", "terminus")
            .ok_or_else(|| VaultError::Database("tidak bisa resolve config dir".into()))?;
        Self::open_at(dirs.data_dir().join("vault.db"))
    }

    /// Buka (atau bikin baru) vault di path spesifik. Dipisah dari
    /// `open_default` supaya gampang ditest (path sementara) dan
    /// gampang dipakai ulang kalau nanti ada fitur "buka vault lain".
    pub fn open_at(db_path: PathBuf) -> Result<Self, VaultError> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| VaultError::Database(format!("gagal bikin direktori vault: {e}")))?;
        }
        let conn = Connection::open(&db_path)
            .map_err(|e| VaultError::Database(format!("gagal buka database: {e}")))?;
        let store = Self { conn, db_path, unlocked_key: None };
        store.migrate()?;
        Ok(store)
    }

    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    fn migrate(&self) -> Result<(), VaultError> {
        self.conn
            .execute_batch(
                "
                CREATE TABLE IF NOT EXISTS vault_meta (
                    id INTEGER PRIMARY KEY CHECK (id = 1),
                    salt BLOB NOT NULL,
                    verifier BLOB NOT NULL
                );
                CREATE TABLE IF NOT EXISTS groups (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    subtitle TEXT,
                    parent_id TEXT
                );
                CREATE TABLE IF NOT EXISTS profiles (
                    id TEXT PRIMARY KEY,
                    label TEXT NOT NULL,
                    host TEXT NOT NULL,
                    port INTEGER NOT NULL,
                    username TEXT NOT NULL,
                    kind TEXT NOT NULL,
                    auth_method TEXT NOT NULL,
                    credential_id TEXT,
                    group_id TEXT,
                    tags TEXT NOT NULL DEFAULT '[]'
                );
                -- `data` = nonce(12 byte) || ciphertext+tag, sudah dalam satu
                -- blob (lihat crypto::encrypt) supaya tidak ada dua kolom yang
                -- bisa 'kegeser' relatif satu sama lain kalau ada bug migrasi.
                CREATE TABLE IF NOT EXISTS secrets (
                    credential_id TEXT PRIMARY KEY,
                    data BLOB NOT NULL
                );
                CREATE TABLE IF NOT EXISTS known_hosts (
                    host TEXT NOT NULL,
                    port INTEGER NOT NULL,
                    key_line TEXT NOT NULL,
                    PRIMARY KEY (host, port)
                );
                -- Identity tersimpan (pasangan username+password
                -- TERPISAH dari host, lihat `terminus_core::Identity`).
                -- `credential_id` NOT NULL (beda dari `profiles` yang
                -- boleh NULL buat `AuthMethod::Agent`) — Identity tanpa
                -- password sama sekali tidak ada gunanya, jadi selalu
                -- ada satu baris `secrets` yang terkait.
                CREATE TABLE IF NOT EXISTS identities (
                    id TEXT PRIMARY KEY,
                    label TEXT NOT NULL,
                    username TEXT NOT NULL,
                    credential_id TEXT NOT NULL
                );
                ",
            )
            .map_err(|e| VaultError::Database(format!("migrasi gagal: {e}")))?;

        // `CREATE TABLE IF NOT EXISTS` di atas TIDAK menambah kolom baru
        // ke tabel `profiles` yang sudah ada dari instalasi sebelum fitur
        // "tema per-host persist" ini — perlu migrasi kolom terpisah,
        // dijaga idempotent (aman dipanggil ulang tiap start app) lewat
        // cek `PRAGMA table_info` dulu sebelum `ALTER TABLE`.
        self.add_column_if_missing("profiles", "terminal_theme", "TEXT")?;

        Ok(())
    }

    /// Tambah satu kolom ke tabel yang SUDAH ADA, hanya kalau belum ada
    /// kolom dengan nama itu — SQLite tidak punya `ADD COLUMN IF NOT
    /// EXISTS` bawaan, jadi dicek manual lewat `PRAGMA table_info`.
    fn add_column_if_missing(&self, table: &str, column: &str, sql_type: &str) -> Result<(), VaultError> {
        let mut stmt = self
            .conn
            .prepare(&format!("PRAGMA table_info({table})"))
            .map_err(|e| VaultError::Database(e.to_string()))?;
        let exists = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .map_err(|e| VaultError::Database(e.to_string()))?
            .filter_map(|r| r.ok())
            .any(|name| name == column);
        drop(stmt);
        if !exists {
            self.conn
                .execute(&format!("ALTER TABLE {table} ADD COLUMN {column} {sql_type}"), [])
                .map_err(|e| VaultError::Database(format!("gagal tambah kolom {column}: {e}")))?;
        }
        Ok(())
    }

    // --- Lifecycle master password ---

    /// Sudah pernah di-`initialize()` sebelumnya? Dipakai UI buat
    /// memutuskan tampilkan dialog "Buat Master Password" (belum
    /// pernah) vs "Unlock Vault" (sudah pernah).
    pub fn is_initialized(&self) -> Result<bool, VaultError> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM vault_meta WHERE id = 1", [], |row| row.get(0))
            .map_err(|e| VaultError::Database(e.to_string()))?;
        Ok(count > 0)
    }

    /// Setup pertama kali: generate salt baru, derive key, simpan
    /// verifier. Vault langsung ke-unlock setelah ini (user baru saja
    /// mengetik password-nya, tidak perlu ketik ulang).
    pub fn initialize(&mut self, master_password: &str) -> Result<(), VaultError> {
        if self.is_initialized()? {
            return Err(VaultError::Database("vault sudah pernah dibuat".into()));
        }
        let salt = crypto::generate_salt();
        let key = crypto::derive_key(master_password, &salt)?;
        let verifier = crypto::encrypt(&key, VAULT_VERIFIER_MARKER)?;

        self.conn
            .execute(
                "INSERT INTO vault_meta (id, salt, verifier) VALUES (1, ?1, ?2)",
                params![salt.to_vec(), verifier],
            )
            .map_err(|e| VaultError::Database(e.to_string()))?;

        self.unlocked_key = Some(key);
        Ok(())
    }

    pub fn unlock(&mut self, master_password: &str) -> Result<(), VaultError> {
        let (salt, verifier): (Vec<u8>, Vec<u8>) = self
            .conn
            .query_row("SELECT salt, verifier FROM vault_meta WHERE id = 1", [], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .optional()
            .map_err(|e| VaultError::Database(e.to_string()))?
            .ok_or_else(|| VaultError::Database("vault belum diinisialisasi".into()))?;

        let key = crypto::derive_key(master_password, &salt)?;
        let decrypted = crypto::decrypt(&key, &verifier).map_err(|_| VaultError::WrongPassword)?;
        if decrypted != VAULT_VERIFIER_MARKER {
            return Err(VaultError::WrongPassword);
        }

        self.unlocked_key = Some(key);
        Ok(())
    }

    pub fn lock(&mut self) {
        self.unlocked_key = None;
    }

    pub fn is_unlocked(&self) -> bool {
        self.unlocked_key.is_some()
    }

    fn require_key(&self) -> Result<[u8; 32], VaultError> {
        self.unlocked_key.ok_or(VaultError::Locked)
    }

    // --- Host profile CRUD (metadata plaintext, tidak butuh unlock) ---

    pub fn list_ungrouped_profiles(&self) -> Result<Vec<HostProfile>, VaultError> {
        self.query_profiles("WHERE group_id IS NULL", [])
    }

    pub fn list_profiles_in_group(&self, group_id: Uuid) -> Result<Vec<HostProfile>, VaultError> {
        self.query_profiles("WHERE group_id = ?1", params![group_id.to_string()])
    }

    pub fn list_all_profiles(&self) -> Result<Vec<HostProfile>, VaultError> {
        self.query_profiles("", [])
    }

    fn query_profiles(
        &self,
        clause: &str,
        query_params: impl rusqlite::Params,
    ) -> Result<Vec<HostProfile>, VaultError> {
        let sql = format!(
            "SELECT id, label, host, port, username, kind, auth_method, credential_id, group_id, tags, terminal_theme \
             FROM profiles {clause}"
        );
        let mut stmt = self.conn.prepare(&sql).map_err(|e| VaultError::Database(e.to_string()))?;
        let rows = stmt
            .query_map(query_params, Self::row_to_profile)
            .map_err(|e| VaultError::Database(e.to_string()))?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(|e| VaultError::Database(e.to_string()))
    }

    fn row_to_profile(row: &Row) -> rusqlite::Result<HostProfile> {
        let id: String = row.get(0)?;
        let label: String = row.get(1)?;
        let host: String = row.get(2)?;
        let port: i64 = row.get(3)?;
        let username: String = row.get(4)?;
        let kind: String = row.get(5)?;
        let auth_method: String = row.get(6)?;
        let credential_id: Option<String> = row.get(7)?;
        let group_id: Option<String> = row.get(8)?;
        let tags_json: String = row.get(9)?;
        let terminal_theme: Option<String> = row.get(10)?;

        let kind = match kind.as_str() {
            "cisco_ios" => ConnectionKind::CiscoIos,
            _ => ConnectionKind::Ssh,
        };
        let parsed_credential_id = credential_id.as_deref().and_then(|s| Uuid::parse_str(s).ok());
        let auth = match auth_method.as_str() {
            "password" => AuthMethod::Password {
                credential_id: parsed_credential_id.unwrap_or_default(),
            },
            "private_key" => AuthMethod::PrivateKey {
                credential_id: parsed_credential_id.unwrap_or_default(),
            },
            _ => AuthMethod::Agent,
        };
        let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();

        Ok(HostProfile {
            id: Uuid::parse_str(&id).unwrap_or_default(),
            label,
            host,
            port: port as u16,
            username,
            kind,
            auth,
            group_id: group_id.and_then(|s| Uuid::parse_str(&s).ok()),
            tags,
            terminal_theme,
        })
    }

    pub fn save_profile(&mut self, profile: &HostProfile) -> Result<(), VaultError> {
        let kind = match profile.kind {
            ConnectionKind::Ssh => "ssh",
            ConnectionKind::CiscoIos => "cisco_ios",
        };
        let (auth_method, credential_id): (&str, Option<String>) = match &profile.auth {
            AuthMethod::Password { credential_id } => ("password", Some(credential_id.to_string())),
            AuthMethod::PrivateKey { credential_id } => ("private_key", Some(credential_id.to_string())),
            AuthMethod::Agent => ("agent", None),
        };
        let tags_json = serde_json::to_string(&profile.tags).unwrap_or_else(|_| "[]".into());

        self.conn
            .execute(
                "INSERT INTO profiles (id, label, host, port, username, kind, auth_method, credential_id, group_id, tags, terminal_theme)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
                 ON CONFLICT(id) DO UPDATE SET
                   label = excluded.label, host = excluded.host, port = excluded.port,
                   username = excluded.username, kind = excluded.kind, auth_method = excluded.auth_method,
                   credential_id = excluded.credential_id, group_id = excluded.group_id, tags = excluded.tags,
                   terminal_theme = excluded.terminal_theme",
                params![
                    profile.id.to_string(),
                    profile.label,
                    profile.host,
                    profile.port as i64,
                    profile.username,
                    kind,
                    auth_method,
                    credential_id,
                    profile.group_id.map(|g| g.to_string()),
                    tags_json,
                    profile.terminal_theme,
                ],
            )
            .map_err(|e| VaultError::Database(e.to_string()))?;
        Ok(())
    }

    pub fn delete_profile(&mut self, id: Uuid) -> Result<(), VaultError> {
        // Ambil `credential_id`-nya DULU sebelum baris profile-nya
        // hilang, biar secret terenkripsi yang terkait ikut dibersihkan
        // — tanpa ini, secret-nya nyangkut selamanya di tabel `secrets`
        // (orphaned, tidak pernah kebaca lagi tapi tetap ada).
        let credential_id: Option<String> = self
            .conn
            .query_row("SELECT credential_id FROM profiles WHERE id = ?1", params![id.to_string()], |row| {
                row.get(0)
            })
            .ok();
        self.conn
            .execute("DELETE FROM profiles WHERE id = ?1", params![id.to_string()])
            .map_err(|e| VaultError::Database(e.to_string()))?;
        if let Some(credential_id) = credential_id {
            self.conn
                .execute("DELETE FROM secrets WHERE credential_id = ?1", params![credential_id])
                .map_err(|e| VaultError::Database(e.to_string()))?;
        }
        Ok(())
    }

    // --- Group CRUD ---

    pub fn list_groups(&self) -> Result<Vec<HostGroup>, VaultError> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, subtitle, parent_id FROM groups")
            .map_err(|e| VaultError::Database(e.to_string()))?;
        let rows = stmt
            .query_map([], |row| {
                let id: String = row.get(0)?;
                let name: String = row.get(1)?;
                let subtitle: Option<String> = row.get(2)?;
                let parent_id: Option<String> = row.get(3)?;
                Ok(HostGroup {
                    id: Uuid::parse_str(&id).unwrap_or_default(),
                    name,
                    subtitle,
                    parent_id: parent_id.and_then(|s| Uuid::parse_str(&s).ok()),
                })
            })
            .map_err(|e| VaultError::Database(e.to_string()))?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(|e| VaultError::Database(e.to_string()))
    }

    pub fn save_group(&mut self, group: &HostGroup) -> Result<(), VaultError> {
        self.conn
            .execute(
                "INSERT INTO groups (id, name, subtitle, parent_id) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(id) DO UPDATE SET name = excluded.name, subtitle = excluded.subtitle, parent_id = excluded.parent_id",
                params![
                    group.id.to_string(),
                    group.name,
                    group.subtitle,
                    group.parent_id.map(|p| p.to_string()),
                ],
            )
            .map_err(|e| VaultError::Database(e.to_string()))?;
        Ok(())
    }

    /// Hapus grup. Host di dalamnya TIDAK ikut kehapus — cuma jadi
    /// ungrouped lagi (`group_id` di-NULL-kan), lebih aman daripada
    /// diam-diam menghapus host orang.
    /// Hapus grup BESERTA semua host di dalamnya (cascade) — TERMASUK
    /// password terenkripsi tiap host itu. Perilaku ini SENGAJA diubah
    /// (sebelumnya: host di dalam grup cuma jadi ungrouped, TIDAK ikut
    /// terhapus) atas permintaan eksplisit user — tegaskan lagi di sini
    /// karena ini operasi DESTRUKTIF & TIDAK BISA DIBATALKAN, makanya
    /// caller (`crates/app/src/state.rs::on_group_delete_requested`)
    /// WAJIB lewat dialog konfirmasi dulu (sama pola dengan hapus host
    /// satu-satu), bukan langsung dipanggil dari klik tombol "×".
    pub fn delete_group(&mut self, id: Uuid) -> Result<(), VaultError> {
        let tx = self.conn.transaction().map_err(|e| VaultError::Database(e.to_string()))?;

        let credential_ids: Vec<String> = {
            let mut stmt = tx
                .prepare("SELECT credential_id FROM profiles WHERE group_id = ?1 AND credential_id IS NOT NULL")
                .map_err(|e| VaultError::Database(e.to_string()))?;
            let rows = stmt
                .query_map(params![id.to_string()], |row| row.get::<_, String>(0))
                .map_err(|e| VaultError::Database(e.to_string()))?;
            rows.collect::<Result<Vec<_>, _>>().map_err(|e| VaultError::Database(e.to_string()))?
        };

        tx.execute("DELETE FROM profiles WHERE group_id = ?1", params![id.to_string()])
            .map_err(|e| VaultError::Database(e.to_string()))?;
        for credential_id in &credential_ids {
            tx.execute("DELETE FROM secrets WHERE credential_id = ?1", params![credential_id])
                .map_err(|e| VaultError::Database(e.to_string()))?;
        }
        tx.execute("DELETE FROM groups WHERE id = ?1", params![id.to_string()])
            .map_err(|e| VaultError::Database(e.to_string()))?;
        tx.commit().map_err(|e| VaultError::Database(e.to_string()))?;
        Ok(())
    }

    // --- Secret (terenkripsi, wajib unlock) ---

    pub fn store_secret(&mut self, credential_id: Uuid, plaintext: &[u8]) -> Result<(), VaultError> {
        let key = self.require_key()?;
        let ciphertext = crypto::encrypt(&key, plaintext)?;
        self.conn
            .execute(
                "INSERT INTO secrets (credential_id, data) VALUES (?1, ?2)
                 ON CONFLICT(credential_id) DO UPDATE SET data = excluded.data",
                params![credential_id.to_string(), ciphertext],
            )
            .map_err(|e| VaultError::Database(e.to_string()))?;
        Ok(())
    }

    pub fn read_secret(&self, credential_id: Uuid) -> Result<Vec<u8>, VaultError> {
        let key = self.require_key()?;
        let ciphertext: Vec<u8> = self
            .conn
            .query_row(
                "SELECT data FROM secrets WHERE credential_id = ?1",
                params![credential_id.to_string()],
                |row| row.get(0),
            )
            .map_err(|e| VaultError::Database(e.to_string()))?;
        crypto::decrypt(&key, &ciphertext)
    }

    pub fn delete_secret(&mut self, credential_id: Uuid) -> Result<(), VaultError> {
        self.conn
            .execute("DELETE FROM secrets WHERE credential_id = ?1", params![credential_id.to_string()])
            .map_err(|e| VaultError::Database(e.to_string()))?;
        Ok(())
    }

    // --- Identity CRUD (pasangan username+password reusable, TERPISAH
    //     dari host — lihat `terminus_core::Identity`). Metadata
    //     (label/username) plaintext, sama seperti `profiles`; password
    //     lewat `credential_id` ke tabel `secrets` yang sama dipakai
    //     host. ---

    pub fn list_identities(&self) -> Result<Vec<Identity>, VaultError> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, label, username, credential_id FROM identities")
            .map_err(|e| VaultError::Database(e.to_string()))?;
        let rows = stmt
            .query_map([], |row| {
                let id: String = row.get(0)?;
                let label: String = row.get(1)?;
                let username: String = row.get(2)?;
                let credential_id: String = row.get(3)?;
                Ok(Identity {
                    id: Uuid::parse_str(&id).unwrap_or_default(),
                    label,
                    username,
                    credential_id: Uuid::parse_str(&credential_id).unwrap_or_default(),
                })
            })
            .map_err(|e| VaultError::Database(e.to_string()))?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(|e| VaultError::Database(e.to_string()))
    }

    pub fn save_identity(&mut self, identity: &Identity) -> Result<(), VaultError> {
        self.conn
            .execute(
                "INSERT INTO identities (id, label, username, credential_id) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(id) DO UPDATE SET label = excluded.label, username = excluded.username, credential_id = excluded.credential_id",
                params![identity.id.to_string(), identity.label, identity.username, identity.credential_id.to_string()],
            )
            .map_err(|e| VaultError::Database(e.to_string()))?;
        Ok(())
    }

    /// Hapus identity BESERTA password terenkripsi terkait (cascade,
    /// sama pola dengan `delete_profile`) — host yang SUDAH dibuat dari
    /// identity ini sebelumnya TIDAK terpengaruh (password-nya sudah
    /// di-copy ke `credential_id` host itu sendiri waktu dipilih, lihat
    /// doc comment `terminus_core::Identity`).
    pub fn delete_identity(&mut self, id: Uuid) -> Result<(), VaultError> {
        let credential_id: Option<String> = self
            .conn
            .query_row("SELECT credential_id FROM identities WHERE id = ?1", params![id.to_string()], |row| {
                row.get(0)
            })
            .ok();
        self.conn
            .execute("DELETE FROM identities WHERE id = ?1", params![id.to_string()])
            .map_err(|e| VaultError::Database(e.to_string()))?;
        if let Some(credential_id) = credential_id {
            self.conn
                .execute("DELETE FROM secrets WHERE credential_id = ?1", params![credential_id])
                .map_err(|e| VaultError::Database(e.to_string()))?;
        }
        Ok(())
    }

    // --- Known hosts (TOFU host-key pinning, metadata plaintext) ---

    /// Baris `known_hosts` yang sudah tersimpan buat `(host, port)` ini,
    /// kalau ada. Dipakai `terminus-ssh-engine::HostKeyStore` (lewat
    /// adapter di `crates/app`) buat verifikasi identitas server.
    pub fn check_host_key(&self, host: &str, port: u16) -> Result<Option<String>, VaultError> {
        self.conn
            .query_row(
                "SELECT key_line FROM known_hosts WHERE host = ?1 AND port = ?2",
                params![host, port as i64],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| VaultError::Database(e.to_string()))
    }

    /// Simpan/replace host key yang dipercaya untuk `(host, port)`.
    /// Dipanggil sekali waktu TOFU (host baru dikenal) ATAU manual
    /// lewat UI manajemen known-hosts nanti (belum ada).
    pub fn trust_host_key(&mut self, host: &str, port: u16, key_line: &str) -> Result<(), VaultError> {
        self.conn
            .execute(
                "INSERT INTO known_hosts (host, port, key_line) VALUES (?1, ?2, ?3)
                 ON CONFLICT(host, port) DO UPDATE SET key_line = excluded.key_line",
                params![host, port as i64, key_line],
            )
            .map_err(|e| VaultError::Database(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store() -> VaultStore {
        let path = std::env::temp_dir().join(format!("terminus-vault-test-{}.db", Uuid::new_v4()));
        VaultStore::open_at(path).unwrap()
    }

    #[test]
    fn initialize_lalu_unlock_dengan_password_benar() {
        let mut store = temp_store();
        assert!(!store.is_initialized().unwrap());
        store.initialize("password-kuat-123").unwrap();
        assert!(store.is_unlocked());

        store.lock();
        assert!(!store.is_unlocked());
        store.unlock("password-kuat-123").unwrap();
        assert!(store.is_unlocked());
    }

    #[test]
    fn unlock_dengan_password_salah_ditolak() {
        let mut store = temp_store();
        store.initialize("password-benar").unwrap();
        store.lock();
        let err = store.unlock("password-salah").unwrap_err();
        assert!(matches!(err, VaultError::WrongPassword));
    }

    #[test]
    fn secret_tersimpan_terenkripsi_di_kolom_blob() {
        let mut store = temp_store();
        store.initialize("password-kuat-123").unwrap();

        let credential_id = Uuid::new_v4();
        store.store_secret(credential_id, b"password-server-rahasia").unwrap();

        // Baca langsung dari kolom `data` tanpa lewat API decrypt —
        // buktikan isinya BUKAN plaintext.
        let raw: Vec<u8> = store
            .conn
            .query_row(
                "SELECT data FROM secrets WHERE credential_id = ?1",
                params![credential_id.to_string()],
                |row| row.get(0),
            )
            .unwrap();
        assert_ne!(raw, b"password-server-rahasia".to_vec());

        let decrypted = store.read_secret(credential_id).unwrap();
        assert_eq!(decrypted, b"password-server-rahasia");
    }

    #[test]
    fn read_secret_tanpa_unlock_ditolak() {
        let mut store = temp_store();
        store.initialize("password-kuat-123").unwrap();
        let credential_id = Uuid::new_v4();
        store.store_secret(credential_id, b"rahasia").unwrap();
        store.lock();

        let err = store.read_secret(credential_id).unwrap_err();
        assert!(matches!(err, VaultError::Locked));
    }

    /// Perilaku `delete_group` SENGAJA diubah atas permintaan eksplisit
    /// user: dulu host di dalam grup cuma jadi ungrouped (nama test ini
    /// sebelumnya `hapus_grup_tidak_ikut_hapus_host_di_dalamnya`),
    /// SEKARANG host-nya BENERAN ikut terhapus (cascade) — TERMASUK
    /// password terenkripsinya (dibuktikan `read_secret` gagal
    /// setelahnya, bukan cuma cek profile-nya hilang).
    #[test]
    fn hapus_grup_ikut_hapus_host_dan_secret_di_dalamnya() {
        let mut store = temp_store();
        store.initialize("password-kuat-123").unwrap();

        let group = HostGroup { id: Uuid::new_v4(), name: "Production".into(), subtitle: None, parent_id: None };
        store.save_group(&group).unwrap();

        let credential_id = Uuid::new_v4();
        store.store_secret(credential_id, b"password-server-rahasia").unwrap();
        let profile = HostProfile {
            id: Uuid::new_v4(),
            label: "prod-db-01".into(),
            host: "10.0.0.5".into(),
            port: 22,
            username: "root".into(),
            kind: ConnectionKind::Ssh,
            auth: AuthMethod::Password { credential_id },
            group_id: Some(group.id),
            tags: vec![],
            terminal_theme: None,
        };
        store.save_profile(&profile).unwrap();

        // Host DI LUAR grup ini harus tetap utuh — buktikan cascade-nya
        // cuma nyasar host di DALAM grup yang dihapus, bukan semuanya.
        let untouched = HostProfile {
            id: Uuid::new_v4(),
            label: "standalone".into(),
            host: "10.0.0.9".into(),
            port: 22,
            username: "root".into(),
            kind: ConnectionKind::Ssh,
            auth: AuthMethod::Password { credential_id: Uuid::new_v4() },
            group_id: None,
            tags: vec![],
            terminal_theme: None,
        };
        store.save_profile(&untouched).unwrap();

        store.delete_group(group.id).unwrap();

        assert!(store.list_groups().unwrap().is_empty(), "grup harus hilang");
        let all = store.list_all_profiles().unwrap();
        assert_eq!(all.len(), 1, "host DI DALAM grup harus ikut terhapus, cuma host di luar yang tersisa");
        assert_eq!(all[0].id, untouched.id);
        assert!(
            matches!(store.read_secret(credential_id), Err(VaultError::Database(_))),
            "password terenkripsi host yang terhapus harus ikut dibersihkan, bukan nyangkut"
        );
    }

    /// Bukti langsung fitur "Identity tersimpan": simpan, list, dan
    /// hapus identity BESERTA cascade delete password terenkripsinya
    /// (sama pola cascade dengan `delete_profile`) — identity LAIN yang
    /// tidak dihapus harus tetap utuh.
    #[test]
    fn identity_crud_dan_hapus_ikut_hapus_secret() {
        let mut store = temp_store();
        store.initialize("password-kuat-123").unwrap();

        let cred_a = Uuid::new_v4();
        store.store_secret(cred_a, b"password-noc-router").unwrap();
        let identity_a =
            Identity { id: Uuid::new_v4(), label: "NOC Router".into(), username: "bro-noc".into(), credential_id: cred_a };
        store.save_identity(&identity_a).unwrap();

        let cred_b = Uuid::new_v4();
        store.store_secret(cred_b, b"password-lain").unwrap();
        let identity_b =
            Identity { id: Uuid::new_v4(), label: "Server Admin".into(), username: "admin".into(), credential_id: cred_b };
        store.save_identity(&identity_b).unwrap();

        let listed = store.list_identities().unwrap();
        assert_eq!(listed.len(), 2);
        assert!(listed.iter().any(|i| i.id == identity_a.id && i.username == "bro-noc"));

        store.delete_identity(identity_a.id).unwrap();
        let after_delete = store.list_identities().unwrap();
        assert_eq!(after_delete.len(), 1, "cuma identity yang dihapus yang hilang");
        assert_eq!(after_delete[0].id, identity_b.id);
        assert!(
            matches!(store.read_secret(cred_a), Err(VaultError::Database(_))),
            "password identity yang dihapus harus ikut dibersihkan, bukan nyangkut"
        );
        assert_eq!(
            store.read_secret(cred_b).unwrap(),
            b"password-lain",
            "password identity LAIN yang tidak dihapus harus tetap utuh"
        );
    }

    /// Bukti langsung fitur "tema per-host lintas restart": `terminal_
    /// theme` harus ikut ke-roundtrip lewat `save_profile`/`list_all_
    /// profiles` persis kolom lain, TERMASUK simulasi vault LAMA (dibuat
    /// sebelum kolom ini ada) yang di-buka ulang lewat `open_at` — buat
    /// buktikan migrasi `ALTER TABLE ADD COLUMN` (`add_column_if_
    /// missing`) beneran idempotent & tidak merusak data lama.
    #[test]
    fn terminal_theme_persist_lintas_buka_ulang_vault() {
        // Sama pola dengan `temp_store()` (bukan lewat helper itu
        // langsung) karena test ini butuh PATH-nya juga buat `open_at`
        // ULANG beberapa kali (simulasi restart app) — `temp_store()`
        // sendiri tidak mengembalikan path-nya.
        let db_path = std::env::temp_dir().join(format!("terminus-vault-test-{}.db", Uuid::new_v4()));

        let mut store = VaultStore::open_at(db_path.clone()).unwrap();
        store.initialize("password-kuat-123").unwrap();
        let credential_id = Uuid::new_v4();
        store.store_secret(credential_id, b"secret").unwrap();
        let profile = HostProfile {
            id: Uuid::new_v4(),
            label: "router-core".into(),
            host: "10.1.1.1".into(),
            port: 22,
            username: "admin".into(),
            kind: ConnectionKind::Ssh,
            auth: AuthMethod::Password { credential_id },
            group_id: None,
            tags: vec![],
            terminal_theme: None,
        };
        store.save_profile(&profile).unwrap();
        drop(store);

        // "Buka ulang" vault ala restart app — profil BELUM punya tema
        // (`None`), sama seperti host yang belum pernah eksplisit ganti
        // tema.
        let mut store = VaultStore::open_at(db_path.clone()).unwrap();
        let loaded = store.list_all_profiles().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].terminal_theme, None, "tema belum pernah diset, harus None");

        // Simulasikan user ganti tema (persis alur `on_terminal_theme_
        // changed` di crates/app/src/state.rs) lalu simpan ulang profil.
        let mut updated = loaded[0].clone();
        updated.terminal_theme = Some("Solarized Dark".to_string());
        store.save_profile(&updated).unwrap();
        drop(store);

        // Buka ulang LAGI — tema harus tetap "Solarized Dark", bukti
        // fitur ini beneran lintas restart (bukan cuma in-memory).
        let store = VaultStore::open_at(db_path).unwrap();
        let reloaded = store.list_all_profiles().unwrap();
        assert_eq!(reloaded.len(), 1);
        assert_eq!(reloaded[0].terminal_theme.as_deref(), Some("Solarized Dark"));
    }
}
