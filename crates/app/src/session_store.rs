//! Penyimpanan refresh token (mode Self-hosted) di database lokal
//! TERENKRIPSI — TERPISAH dari `app_config.json` (plaintext, cuma
//! field non-rahasia) dan TERPISAH dari `vault.db` (beda makna: data
//! host mode Local). Lihat `docs/desktop-selfhosted-integration.md`
//! bagian 2.5 buat rasional & batasan keamanan pendekatan ini —
//! khususnya: key enkripsinya JUGA di disk yang sama (supaya bisa
//! auto-decrypt tanpa minta password lagi tiap start app), jadi ini
//! melindungi dari paparan tidak sengaja, BUKAN dari penyerang yang
//! sudah punya akses penuh ke mesin yang sama.
//!
//! Dua file baru di config dir yang sama dengan `app_config.json`:
//! - `session.key` — 32 byte random, permission 0600 (Unix).
//! - `session.db` — SQLite satu tabel satu baris, isinya blob hasil
//!   `terminus_vault::crypto::encrypt` (TIDAK menulis crypto baru).

// `dead_code` sementara: modul ini belum dipanggil dari `main.rs`/
// `state.rs` (itu Milestone 3, wiring alur login mode Self-hosted di
// `docs/desktop-selfhosted-integration.md`) — dihapus waktu wiring
// itu dikerjakan.
#![allow(dead_code)]

use rand::RngCore;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::PathBuf;

fn dirs() -> anyhow::Result<directories::ProjectDirs> {
    directories::ProjectDirs::from("com", "terminus", "terminus")
        .ok_or_else(|| anyhow::anyhow!("tidak bisa resolve config dir"))
}

fn key_path() -> anyhow::Result<PathBuf> {
    Ok(dirs()?.config_dir().join("session.key"))
}

fn db_path() -> anyhow::Result<PathBuf> {
    Ok(dirs()?.config_dir().join("session.db"))
}

/// Baca `session.key` kalau sudah ada, atau generate+simpan baru (32
/// byte random) kalau belum. Dipanggil otomatis oleh
/// `save_refresh_token`/`load_refresh_token` — tidak perlu dipanggil
/// manual dari luar modul ini.
fn load_or_create_key(config_dir: &std::path::Path) -> anyhow::Result<[u8; 32]> {
    std::fs::create_dir_all(config_dir)?;
    let path = key_path()?;
    if let Ok(existing) = std::fs::read(&path) {
        if existing.len() == 32 {
            let mut key = [0u8; 32];
            key.copy_from_slice(&existing);
            return Ok(key);
        }
        // Panjang tidak sesuai (file korup/diutak-atik manual) -> anggap
        // tidak ada, timpa dengan key baru di bawah. Sesi lama otomatis
        // tidak kebaca lagi (user tinggal login ulang) -- lebih aman
        // daripada crash atau pakai key yang salah bentuk.
    }
    let mut key = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut key);
    std::fs::write(&path, key)?;
    set_owner_only_permissions(&path)?;
    Ok(key)
}

#[cfg(unix)]
fn set_owner_only_permissions(path: &std::path::Path) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_owner_only_permissions(_path: &std::path::Path) -> anyhow::Result<()> {
    // Windows: ACL default per-user profile sudah cukup terpisah dari
    // user lain, tidak ada padanan chmod sederhana yang lintas-versi
    // konsisten -- tidak diberlakukan.
    Ok(())
}

fn open_db() -> anyhow::Result<Connection> {
    let path = db_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(path)?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS session_secret (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            encrypted_token BLOB NOT NULL
        );",
    )?;
    Ok(conn)
}

/// Simpan refresh token (menimpa yang lama kalau ada satu baris).
pub fn save_refresh_token(token: &str) -> anyhow::Result<()> {
    let config_dir = dirs()?.config_dir().to_path_buf();
    let key = load_or_create_key(&config_dir)?;
    let encrypted = terminus_vault::crypto::encrypt(&key, token.as_bytes())
        .map_err(|e| anyhow::anyhow!("gagal enkripsi refresh token: {e}"))?;
    let conn = open_db()?;
    conn.execute(
        "INSERT INTO session_secret (id, encrypted_token) VALUES (1, ?1)
         ON CONFLICT(id) DO UPDATE SET encrypted_token = excluded.encrypted_token",
        params![encrypted],
    )?;
    Ok(())
}

/// Baca refresh token tersimpan. `None` kalau belum pernah login
/// Self-hosted sama sekali di mesin ini (belum ada baris/`session.db`
/// belum ada).
pub fn load_refresh_token() -> anyhow::Result<Option<String>> {
    let path = db_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let config_dir = dirs()?.config_dir().to_path_buf();
    let key = load_or_create_key(&config_dir)?;
    let conn = open_db()?;
    let encrypted: Option<Vec<u8>> = conn
        .query_row("SELECT encrypted_token FROM session_secret WHERE id = 1", [], |row| {
            row.get(0)
        })
        .optional()?;
    match encrypted {
        None => Ok(None),
        Some(blob) => {
            let plaintext = terminus_vault::crypto::decrypt(&key, &blob)
                .map_err(|e| anyhow::anyhow!("gagal dekripsi refresh token: {e}"))?;
            Ok(Some(String::from_utf8(plaintext)?))
        }
    }
}

/// Hapus refresh token tersimpan — dipanggil waktu logout eksplisit
/// (lihat rencana wiring di Milestone 3).
pub fn clear() -> anyhow::Result<()> {
    let path = db_path()?;
    if !path.exists() {
        return Ok(());
    }
    let conn = open_db()?;
    conn.execute("DELETE FROM session_secret", [])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // `dirs()`/`key_path()`/`db_path()` pakai `directories::ProjectDirs`
    // yang path-nya fixed (tidak gampang di-override lewat env var) --
    // jadi test di sini fokus ke roundtrip crypto inti-nya lewat helper
    // lokal berlogic SAMA PERSIS dengan `load_or_create_key` tapi
    // parametrized path sementara, supaya TIDAK menyentuh config dir
    // OS sungguhan (`~/.config/terminus`) waktu `cargo test`.

    #[test]
    fn load_or_create_key_konsisten_dan_roundtrip_crypto() {
        let dir = std::env::temp_dir().join(format!("terminus-session-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();

        fn load_or_create_key_at(path: &std::path::Path) -> anyhow::Result<[u8; 32]> {
            if let Ok(existing) = std::fs::read(path) {
                if existing.len() == 32 {
                    let mut key = [0u8; 32];
                    key.copy_from_slice(&existing);
                    return Ok(key);
                }
            }
            let mut key = [0u8; 32];
            rand::rngs::OsRng.fill_bytes(&mut key);
            std::fs::write(path, key)?;
            Ok(key)
        }

        let key_path = dir.join("session.key");
        let first = load_or_create_key_at(&key_path).unwrap();
        let second = load_or_create_key_at(&key_path).unwrap();
        assert_eq!(first, second, "key harus konsisten dibaca ulang, bukan digenerate ulang tiap panggil");

        let encrypted = terminus_vault::crypto::encrypt(&first, b"refresh-token-contoh").unwrap();
        let decrypted = terminus_vault::crypto::decrypt(&second, &encrypted).unwrap();
        assert_eq!(decrypted, b"refresh-token-contoh");

        std::fs::remove_dir_all(&dir).ok();
    }
}
