//! `terminus-ssh-engine`
//!
//! Wrapper tipis di atas `russh` (implementasi SSH2 pure Rust). Tanggung
//! jawab crate ini CUMA transport layer: connect, auth, buka channel shell,
//! kirim/terima byte stream. Parsing escape sequence VTE ada di
//! `terminus-term-emulator`, bukan di sini — biar concern-nya terpisah.

use terminus_core::HostProfile;
use std::sync::{Arc, Mutex};
use thiserror::Error;

pub mod client;
pub mod session;

pub use session::{SshOutputEvent, SshSession};

#[derive(Debug, Error)]
pub enum SshEngineError {
    #[error("koneksi gagal: {0}")]
    Connect(String),

    #[error("autentikasi gagal untuk user {0}")]
    AuthFailed(String),

    #[error("channel error: {0}")]
    Channel(String),

    /// Host key server BEDA dari yang tersimpan di `known_hosts` —
    /// potensi man-in-the-middle. Sengaja dipisah dari `Connect` biasa
    /// supaya UI bisa kasih pesan yang jelas & tidak menyarankan
    /// "coba lagi" begitu saja (lihat `crates/ssh-engine/src/client.rs`).
    #[error("host key berubah, koneksi ditolak: {0}")]
    HostKeyMismatch(String),
}

/// Sumber kebenaran trust host-key (TOFU). Implementasinya hidup di
/// `crates/app` (menjembatani ke `terminus-vault::VaultStore`), supaya
/// crate ini TIDAK perlu depend langsung ke vault — lihat aturan arah
/// dependency di `docs/architecture.md`.
pub trait HostKeyStore: Send + Sync {
    /// Baris host-key (format sama dengan `PublicKey::to_openssh()`)
    /// yang tersimpan untuk `(host, port)` ini, kalau ada.
    fn lookup(&self, host: &str, port: u16) -> Option<String>;
    /// Dipanggil sekali waktu host key baru pertama kali dipercaya
    /// (trust-on-first-use).
    fn trust(&self, host: &str, port: u16, key_line: String);
}

/// Materi rahasia untuk autentikasi, dioper dari vault ke engine saat
/// runtime saja (tidak pernah disimpan di struct HostProfile).
pub enum SecretMaterial {
    Password(String),
    PrivateKeyPem { pem: String, passphrase: Option<String> },
    Agent,
}

/// Connect + autentikasi SAJA (tanpa buka channel shell) — diekstrak
/// dari `connect()` (di bawah) supaya bisa dipakai ULANG oleh
/// `terminus-sftp-engine`, yang butuh `Handle` yang SAMA (satu koneksi
/// TCP+SSH) tapi buka channel BEDA (`request_subsystem("sftp")`,
/// bukan `request_shell`) — SFTP TIDAK jalan di atas channel shell
/// interaktif, itu channel terpisah dalam koneksi SSH yang sama.
/// `Handle<client::ClientHandler>` HARUS tetap hidup selama sesi
/// (drop = socket ketutup) — caller (mis. `SftpBrowser`) tanggung
/// jawab nyimpennya.
///
/// Catatan v1: cuma `SecretMaterial::Password` yang diimplementasi.
/// Private key & SSH agent menyusul di iterasi berikutnya.
pub async fn connect_handle(
    profile: &HostProfile,
    secret: SecretMaterial,
    host_key_store: Arc<dyn HostKeyStore>,
) -> Result<russh::client::Handle<client::ClientHandler>, SshEngineError> {
    let password = match secret {
        SecretMaterial::Password(pw) => pw,
        SecretMaterial::PrivateKeyPem { .. } => {
            return Err(SshEngineError::Connect("autentikasi private key belum didukung".into()));
        }
        SecretMaterial::Agent => {
            return Err(SshEngineError::Connect("autentikasi lewat SSH agent belum didukung".into()));
        }
    };

    // `rejected_reason` dipakai ClientHandler buat "menitipkan" alasan
    // spesifik kalau `check_server_key` menolak koneksi (host key
    // berubah) — tanpa ini, kita cuma dapat error generik dari russh
    // yang tidak bilang APA sebabnya.
    let rejected_reason = Arc::new(Mutex::new(None));
    let handler = client::ClientHandler {
        host: profile.host.clone(),
        port: profile.port,
        host_key_store,
        rejected_reason: rejected_reason.clone(),
    };

    let config = Arc::new(russh::client::Config::default());
    let mut handle = russh::client::connect(config, (profile.host.as_str(), profile.port), handler)
        .await
        .map_err(|e| {
            if let Some(reason) = rejected_reason.lock().ok().and_then(|mut g| g.take()) {
                SshEngineError::HostKeyMismatch(reason)
            } else {
                SshEngineError::Connect(e.to_string())
            }
        })?;

    let auth_result = handle
        .authenticate_password(profile.username.clone(), password)
        .await
        .map_err(|e| SshEngineError::Connect(e.to_string()))?;

    if !auth_result.success() {
        return Err(SshEngineError::AuthFailed(profile.username.clone()));
    }

    Ok(handle)
}

/// Entry point: buka sesi SSH INTERAKTIF (channel shell) baru dari
/// sebuah `HostProfile`. Kredensial aktual (password/private key)
/// diambil oleh caller dari `terminus-vault` lalu dioper ke sini —
/// engine ini tidak tahu-menahu soal penyimpanan kredensial.
pub async fn connect(
    profile: &HostProfile,
    secret: SecretMaterial,
    host_key_store: Arc<dyn HostKeyStore>,
) -> Result<SshSession, SshEngineError> {
    let handle = connect_handle(profile, secret, host_key_store).await?;

    let mut channel = handle.channel_open_session().await.map_err(|e| SshEngineError::Channel(e.to_string()))?;

    channel
        .request_pty(true, "xterm-256color", 80, 24, 0, 0, &[])
        .await
        .map_err(|e| SshEngineError::Channel(e.to_string()))?;

    channel.request_shell(true).await.map_err(|e| SshEngineError::Channel(e.to_string()))?;

    // Tunggu satu balasan (SSH_MSG_CHANNEL_SUCCESS/FAILURE) sebagai
    // bukti channel shell beneran hidup, bukan cuma "request terkirim".
    match channel.wait().await {
        Some(russh::ChannelMsg::Success) => {}
        Some(russh::ChannelMsg::Failure) => {
            return Err(SshEngineError::Channel("server menolak permintaan shell".into()));
        }
        _ => {
            return Err(SshEngineError::Channel("koneksi terputus sebelum shell siap".into()));
        }
    }

    Ok(SshSession::new(handle, channel))
}
