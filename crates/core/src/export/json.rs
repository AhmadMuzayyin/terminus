//! Skema JSON portable buat backup host TERMASUK password — kebalikan
//! dari `crate::export::securecrt` yang sengaja TANPA password. Dipakai
//! buat pindah host antar mesin / backup manual di LUAR file vault
//! SQLite asli (`vault.db`), makanya passwordnya WAJIB terenkripsi
//! (bukan plaintext) — siapa pun yang buka file `.json` ini masih
//! butuh passphrase backup buat baca isi password-nya.
//!
//! Modul ini MURNI skema data + serialisasi (sejalan dengan filosofi
//! crate ini, "tidak ada I/O") — enkripsi/dekripsi aktualnya (Argon2id +
//! ChaCha20-Poly1305) TIDAK bisa dilakukan di sini karena `terminus-core`
//! sengaja tidak depend ke `terminus-vault` (arahnya kebalik: vault
//! depend ke core, bukan sebaliknya, lihat `Cargo.toml` masing-masing
//! crate). Caller (`crates/app/src/state.rs`) yang panggil
//! `terminus_vault::crypto::{generate_salt, derive_key, encrypt}` buat
//! ngisi `salt` + `encrypted_password` di sini SEBELUM manggil
//! `render`.
//!
//! **Passphrase backup ini SENGAJA terpisah dari master password
//! vault** — file backup ini bisa dipindah/disimpan di tempat lain
//! (mis. cloud storage, USB), jadi tidak boleh otomatis kebuka pakai
//! password vault utama kalau file-nya bocor.

use serde::{Deserialize, Serialize};

/// Versi skema — dicek caller waktu baca balik file backup, biar
/// perubahan format ke depan (kalau ada) bisa dideteksi eksplisit
/// bukan cuma gagal parse yang membingungkan.
pub const CURRENT_VERSION: u32 = 1;

/// Satu host di backup. Field bentuknya SENGAJA mirror
/// `crate::export::securecrt::ExportHost` (label/host/port/username/
/// group_path) plus field yang tidak ada di format XML SecureCRT
/// (tags, terminal_theme, password).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BackupHost {
    pub label: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    /// "ssh" | "cisco_ios" — string biasa (bukan `ConnectionKind`
    /// langsung) supaya skema JSON di file backup TETAP STABIL biarpun
    /// representasi enum Rust internal berubah nanti.
    pub kind: String,
    pub group_path: Vec<String>,
    pub tags: Vec<String>,
    pub terminal_theme: Option<String>,
    /// Ciphertext mentah (layout `nonce (12 byte) || ciphertext+tag`,
    /// lihat `terminus_vault::crypto::encrypt`) — diserialize serde_json
    /// bawaan sebagai array angka (bukan base64), cukup buat blob
    /// sekecil ini, TIDAK butuh dependency baru cuma buat encoding.
    /// `None` kalau host itu belum punya password tersimpan sama
    /// sekali (mis. host hasil Import SecureCRT yang belum diisi
    /// manual lewat panel).
    pub encrypted_password: Option<Vec<u8>>,
}

/// Satu file backup lengkap. `salt` TIDAK RAHASIA (cara kerja Argon2id
/// normal — tanpa passphrase yang benar, salt sendirian tidak berguna
/// buat buka `encrypted_password` manapun) jadi aman disimpan plaintext
/// di sini.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Backup {
    pub version: u32,
    pub salt: Vec<u8>,
    pub hosts: Vec<BackupHost>,
}

/// Render `Backup` jadi teks JSON siap ditulis ke file. `pretty` biar
/// gampang diperiksa manual (mis. lihat daftar host apa saja yang
/// ke-backup) tanpa perlu tool tambahan — passwordnya sendiri tetap
/// tidak terbaca (ciphertext, bukan plaintext).
pub fn render(backup: &Backup) -> String {
    serde_json::to_string_pretty(backup).expect("Backup selalu bisa diserialize (tidak ada NaN/cycle)")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_lalu_parse_ulang_hasilnya_identik() {
        let backup = Backup {
            version: CURRENT_VERSION,
            salt: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
            hosts: vec![
                BackupHost {
                    label: "prod-web-01".to_string(),
                    host: "10.0.0.5".to_string(),
                    port: 22,
                    username: "deploy".to_string(),
                    kind: "ssh".to_string(),
                    group_path: vec![],
                    tags: vec!["prod".to_string()],
                    terminal_theme: Some("Dracula".to_string()),
                    encrypted_password: Some(vec![9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 1, 2, 3]),
                },
                BackupHost {
                    label: "rtr-fwd".to_string(),
                    host: "192.168.1.1".to_string(),
                    port: 22,
                    username: "admin".to_string(),
                    kind: "cisco_ios".to_string(),
                    group_path: vec!["ROUTER".to_string()],
                    tags: vec![],
                    terminal_theme: None,
                    encrypted_password: None,
                },
            ],
        };

        let json = render(&backup);
        let parsed: Backup = serde_json::from_str(&json).expect("hasil render harus JSON valid & sesuai skema");
        assert_eq!(parsed, backup);
    }

    #[test]
    fn parse_json_rusak_gagal_bukan_panik() {
        let err = serde_json::from_str::<Backup>("{not even json").unwrap_err();
        assert!(err.is_syntax() || err.is_data());
    }
}
