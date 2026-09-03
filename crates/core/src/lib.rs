//! `terminus-core`
//!
//! Domain model murni, tidak ada I/O di sini (tidak ada network, tidak ada
//! filesystem, tidak ada UI). Tujuannya supaya crate ini bisa di-unit-test
//! cepat tanpa dependency berat, dan dipakai bareng oleh ssh-engine,
//! sftp-engine, vault, cisco-driver, maupun app (UI layer).

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub mod error;
pub mod import;

pub use error::CoreError;

/// Jenis koneksi yang didukung. Cisco console dipisah dari SSH biasa
/// karena butuh handling khusus (paging, enable mode, dsb) di
/// `terminus-cisco-driver`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionKind {
    Ssh,
    CiscoIos,
    // SerialConsole akan ditambahkan kalau nanti butuh akses via kabel USB-serial.
}

/// Metode autentikasi. Nilai kredensial (password/passphrase) TIDAK
/// disimpan di struct ini — hanya referensi ID ke entry di `terminus-vault`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthMethod {
    Password { credential_id: Uuid },
    PrivateKey { credential_id: Uuid },
    Agent,
}

/// Satu profil koneksi tersimpan (setara "host" di Termius).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostProfile {
    pub id: Uuid,
    pub label: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub kind: ConnectionKind,
    pub auth: AuthMethod,
    pub group_id: Option<Uuid>,
    pub tags: Vec<String>,
}

/// Grup/folder untuk mengorganisir banyak host di sidebar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostGroup {
    pub id: Uuid,
    pub name: String,
    /// Teks bebas pendek buat konteks tambahan (mis. region/environment),
    /// ditampilkan di bawah nama grup di kartu UI. Opsional.
    pub subtitle: Option<String>,
    pub parent_id: Option<Uuid>,
}

/// State satu sesi yang sedang aktif (dipakai UI buat render tab).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionStatus {
    Connecting,
    Connected,
    Disconnected,
    Failed,
}

#[derive(Debug, Clone)]
pub struct ActiveSession {
    pub id: Uuid,
    pub profile_id: Uuid,
    pub status: SessionStatus,
}
