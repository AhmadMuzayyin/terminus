//! `terminus-vault`
//!
//! Penyimpanan lokal untuk `HostProfile`/`HostGroup` (metadata, boleh
//! plaintext di SQLite) dan kredensial (password/private key, WAJIB
//! terenkripsi). Desain:
//!
//! - Master password user -> derive encryption key pakai **Argon2id**
//!   (memory-hard, tahan brute-force offline).
//! - Kredensial dienkripsi per-entry pakai **ChaCha20-Poly1305** (AEAD,
//!   authenticated encryption, aman & cepat, tidak butuh AES-NI hardware
//!   accel — penting buat cross-platform termasuk ARM Mac).
//! - Metadata (label, host, port, username, grup, tag) disimpan
//!   plaintext di SQLite lewat `rusqlite` supaya bisa di-search/filter
//!   cepat tanpa perlu unlock vault dulu.

use thiserror::Error;

pub mod crypto;
pub mod store;

pub use store::VaultStore;

#[derive(Debug, Error)]
pub enum VaultError {
    #[error("vault terkunci, perlu unlock dengan master password")]
    Locked,

    #[error("master password salah")]
    WrongPassword,

    #[error("database error: {0}")]
    Database(String),

    #[error("crypto error: {0}")]
    Crypto(String),
}
