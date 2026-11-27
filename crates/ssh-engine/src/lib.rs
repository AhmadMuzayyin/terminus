//! `terminus-ssh-engine`
//!
//! Wrapper tipis di atas `russh` (implementasi SSH2 pure Rust). Tanggung
//! jawab crate ini CUMA transport layer: connect, auth, buka channel shell,
//! kirim/terima byte stream. Parsing escape sequence VTE ada di
//! `terminus-term-emulator`, bukan di sini — biar concern-nya terpisah.

use terminus_core::HostProfile;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use thiserror::Error;

pub mod client;
pub mod session;

pub use session::{SshOutputEvent, SshSession};

/// Tahap NYATA yang dilewati satu proses connect SSH, dalam urutan
/// kejadiannya. Dipakai buat menarasikan progres ke UI (layar
/// "Connecting") supaya user tahu macetnya di mana — DNS, socket, auth,
/// atau channel — bukan cuma lihat spinner tanpa informasi apa pun.
///
/// Kenapa ini ada di engine, bukan dikarang di UI: hanya engine yang
/// tahu kapan tiap tahap betul-betul selesai. Kalau UI menganimasikan
/// tahap sendiri pakai timer, yang tampil adalah dugaan — dan user yang
/// koneksinya macet di auth akan melihat "menjalankan shell", justru
/// menyesatkan di saat paling butuh informasi akurat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectStage {
    /// Resolusi DNS nama host jadi satu/lebih alamat IP.
    ResolvingAddress,
    /// Buka socket TCP + handshake SSH (pertukaran versi, KEX) +
    /// verifikasi host key. `russh::client::connect` melakukan semuanya
    /// dalam satu panggilan, jadi tidak bisa dipecah lebih halus tanpa
    /// menyentuh internal russh.
    OpeningSocket,
    /// Autentikasi user (v1: password).
    Authenticating,
    /// Buka channel sesi di atas koneksi yang sudah terautentikasi.
    OpeningChannel,
    /// Minta PTY (ukuran awal 80x24, di-resize caller setelahnya).
    RequestingPty,
    /// Minta shell interaktif + tunggu SSH_MSG_CHANNEL_SUCCESS.
    StartingShell,
}

impl ConnectStage {
    /// Semua tahap dalam urutan kejadian — UI memakai ini buat
    /// menggambar daftar tahap LENGKAP sejak awal (tahap yang belum
    /// jalan tampil redup), bukan menambah baris satu per satu. Dengan
    /// begitu user bisa melihat sisa perjalanan, bukan cuma posisi
    /// sekarang.
    pub const ALL: [ConnectStage; 6] = [
        ConnectStage::ResolvingAddress,
        ConnectStage::OpeningSocket,
        ConnectStage::Authenticating,
        ConnectStage::OpeningChannel,
        ConnectStage::RequestingPty,
        ConnectStage::StartingShell,
    ];

    /// Posisi tahap ini di `ALL`. Dipakai UI buat menentukan tahap mana
    /// yang sudah lewat (selesai) dan mana yang belum.
    pub fn index(self) -> usize {
        ConnectStage::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }

    /// Label singkat untuk ditampilkan ke user.
    pub fn label(self) -> &'static str {
        match self {
            ConnectStage::ResolvingAddress => "Mencari alamat host",
            ConnectStage::OpeningSocket => "Membuka koneksi & verifikasi host key",
            ConnectStage::Authenticating => "Autentikasi",
            ConnectStage::OpeningChannel => "Membuka channel",
            ConnectStage::RequestingPty => "Menyiapkan terminal (PTY)",
            ConnectStage::StartingShell => "Menjalankan shell",
        }
    }
}

/// Penerima laporan progres. `String` opsional = detail tahap itu
/// (mis. alamat IP yang sedang dicoba), ditampilkan kecil di bawah
/// label tahap.
///
/// `Arc<dyn Fn>` dan bukan channel supaya caller bebas memilih: kirim
/// ke `slint::invoke_from_event_loop`, tulis ke log, atau abaikan.
/// Engine tidak tahu-menahu soal UI.
pub type ProgressSink = Arc<dyn Fn(ConnectStage, Option<String>) + Send + Sync>;

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

    // --- Di bawah ini: kesalahan KONFIGURASI, bukan kegagalan jaringan.
    // Dipisah karena perlakuan UI-nya berbeda total: "coba lagi" tidak
    // ada gunanya sebelum user memperbaiki data host-nya dulu. Tanpa
    // pemisahan ini, host yang alamatnya kosong tampil sebagai "koneksi
    // gagal" dan user mengulang-ulang tanpa hasil.
    #[error("alamat host belum diisi")]
    MissingHost,

    #[error("username belum diisi")]
    MissingUsername,

    #[error("port tidak valid: {0}")]
    InvalidPort(u16),

    /// Nama host tidak bisa diresolusi. Dipisah dari `Connect` supaya UI
    /// bisa mengarahkan ke salah ejaan/DNS, bukan ke server yang mati.
    #[error("tidak bisa menemukan alamat untuk {host}: {reason}")]
    DnsFailed { host: String, reason: String },

    /// Semua alamat hasil resolusi sudah dicoba dan gagal. `tried`
    /// penting ditampilkan: host dengan record A + AAAA sekaligus sering
    /// gagal hanya di salah satu family, dan angka inilah yang
    /// membedakan "server mati" dari "IPv6 saya rusak".
    #[error("tidak ada alamat yang bisa dihubungi untuk {host} (dicoba {tried} alamat), terakhir: {last}")]
    AllAddressesFailed { host: String, tried: usize, last: String },
}

impl SshEngineError {
    /// Tahap tempat kegagalan ini terjadi, kalau bisa ditentukan.
    /// Dipakai UI buat menandai baris tahap mana yang merah.
    pub fn stage(&self) -> Option<ConnectStage> {
        match self {
            SshEngineError::MissingHost
            | SshEngineError::MissingUsername
            | SshEngineError::InvalidPort(_) => None,
            SshEngineError::DnsFailed { .. } => Some(ConnectStage::ResolvingAddress),
            SshEngineError::Connect(_)
            | SshEngineError::AllAddressesFailed { .. }
            | SshEngineError::HostKeyMismatch(_) => Some(ConnectStage::OpeningSocket),
            SshEngineError::AuthFailed(_) => Some(ConnectStage::Authenticating),
            SshEngineError::Channel(_) => Some(ConnectStage::OpeningChannel),
        }
    }

    /// Judul pendek untuk header pesan error.
    pub fn title(&self) -> &'static str {
        match self {
            SshEngineError::MissingHost
            | SshEngineError::MissingUsername
            | SshEngineError::InvalidPort(_) => "Data host belum lengkap",
            SshEngineError::DnsFailed { .. } => "Host tidak ditemukan",
            SshEngineError::Connect(_) | SshEngineError::AllAddressesFailed { .. } => {
                "Tidak bisa menghubungi server"
            }
            SshEngineError::HostKeyMismatch(_) => "Host key berubah",
            SshEngineError::AuthFailed(_) => "Autentikasi gagal",
            SshEngineError::Channel(_) => "Server menolak membuka shell",
        }
    }

    /// Saran langkah konkret. Sengaja TIDAK semua error punya saran —
    /// saran mengambang ("periksa jaringan Anda") lebih buruk daripada
    /// tidak ada, karena cuma menyuruh user menebak.
    pub fn hint(&self) -> Option<&'static str> {
        match self {
            SshEngineError::MissingHost => Some("Isi alamat host di panel Host Details."),
            SshEngineError::MissingUsername => Some("Isi username di panel Host Details."),
            SshEngineError::InvalidPort(_) => Some("Port harus antara 1 dan 65535."),
            SshEngineError::DnsFailed { .. } => {
                Some("Periksa ejaan nama host, atau pakai alamat IP langsung.")
            }
            SshEngineError::AllAddressesFailed { .. } => {
                Some("Server mungkin mati, atau firewall memblokir port tersebut.")
            }
            SshEngineError::HostKeyMismatch(_) => Some(
                "JANGAN sambung ulang sebelum yakin. Ini bisa berarti server diganti — \
                 atau ada yang menyadap koneksi Anda.",
            ),
            SshEngineError::AuthFailed(_) => {
                Some("Periksa username, password, atau identity yang dipakai.")
            }
            _ => None,
        }
    }

    /// Apakah tombol "Coba lagi" pantas ditawarkan.
    ///
    /// `HostKeyMismatch` sengaja `false` MESKIPUN secara teknis bisa
    /// diulang: menawarkan "coba lagi" pada peringatan MITM melatih user
    /// menekannya refleks, dan itu menghapus gunanya peringatan tersebut.
    /// Error konfigurasi juga `false`, karena mengulang tanpa
    /// membetulkan datanya pasti gagal lagi.
    pub fn is_retryable(&self) -> bool {
        match self {
            SshEngineError::HostKeyMismatch(_)
            | SshEngineError::MissingHost
            | SshEngineError::MissingUsername
            | SshEngineError::InvalidPort(_) => false,
            _ => true,
        }
    }
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

/// Lapor progres kalau caller memang minta; no-op kalau tidak.
fn report(progress: Option<&ProgressSink>, stage: ConnectStage, detail: Option<String>) {
    if let Some(sink) = progress {
        sink(stage, detail);
    }
}

/// Validasi data host SEBELUM menyentuh jaringan, supaya pesannya
/// spesifik per field — bukan satu "koneksi gagal" untuk semua sebab.
fn validate(profile: &HostProfile) -> Result<(), SshEngineError> {
    if profile.host.trim().is_empty() {
        return Err(SshEngineError::MissingHost);
    }
    if profile.username.trim().is_empty() {
        return Err(SshEngineError::MissingUsername);
    }
    if profile.port == 0 {
        return Err(SshEngineError::InvalidPort(profile.port));
    }
    Ok(())
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
///
/// Versi tanpa laporan progres — tetap ada supaya caller lama
/// (`terminus-sftp-engine`) tidak perlu ikut berubah.
pub async fn connect_handle(
    profile: &HostProfile,
    secret: SecretMaterial,
    host_key_store: Arc<dyn HostKeyStore>,
) -> Result<russh::client::Handle<client::ClientHandler>, SshEngineError> {
    connect_handle_with_progress(profile, secret, host_key_store, None).await
}

/// Seperti `connect_handle`, tapi melaporkan tiap tahap ke `progress`.
pub async fn connect_handle_with_progress(
    profile: &HostProfile,
    secret: SecretMaterial,
    host_key_store: Arc<dyn HostKeyStore>,
    progress: Option<&ProgressSink>,
) -> Result<russh::client::Handle<client::ClientHandler>, SshEngineError> {
    validate(profile)?;

    let password = match secret {
        SecretMaterial::Password(pw) => pw,
        SecretMaterial::PrivateKeyPem { .. } => {
            return Err(SshEngineError::Connect("autentikasi private key belum didukung".into()));
        }
        SecretMaterial::Agent => {
            return Err(SshEngineError::Connect("autentikasi lewat SSH agent belum didukung".into()));
        }
    };

    // --- Tahap 1: resolusi DNS, TERPISAH dari connect.
    //
    // `russh::client::connect` sebetulnya bisa menerima `(host, port)`
    // dan meresolusi sendiri. Kita resolusi manual lebih dulu karena dua
    // alasan yang keduanya kelihatan ke user:
    //
    //   1. Gagal DNS jadi bisa dibedakan dari gagal TCP — penyebab dan
    //      solusinya beda jauh (salah ejaan vs server mati).
    //   2. Kita dapat SEMUA alamat, dan bisa mencoba satu per satu. Host
    //      dengan record A + AAAA di jaringan yang IPv6-nya rusak akan
    //      menggantung kalau cuma alamat pertama yang dicoba — persis
    //      kelas bug yang bikin "timeout misterius" padahal servernya
    //      sehat.
    report(progress, ConnectStage::ResolvingAddress, Some(profile.host.clone()));
    let target = format!("{}:{}", profile.host, profile.port);
    let addrs: Vec<SocketAddr> = tokio::net::lookup_host(&target)
        .await
        .map_err(|e| SshEngineError::DnsFailed { host: profile.host.clone(), reason: e.to_string() })?
        .collect();

    if addrs.is_empty() {
        return Err(SshEngineError::DnsFailed {
            host: profile.host.clone(),
            reason: "resolusi tidak menghasilkan alamat".into(),
        });
    }

    // --- Tahap 2: socket + handshake + host key, dicoba per alamat.
    let rejected_reason = Arc::new(Mutex::new(None));
    let mut last_error: Option<String> = None;
    let mut connected = None;

    for addr in &addrs {
        report(progress, ConnectStage::OpeningSocket, Some(addr.to_string()));

        // Handler dibuat ulang tiap percobaan: `russh::client::connect`
        // mengambil alih (move) handler-nya, jadi tidak bisa dipakai dua
        // kali. Murah — isinya cuma clone string + Arc.
        let handler = client::ClientHandler {
            host: profile.host.clone(),
            port: profile.port,
            host_key_store: host_key_store.clone(),
            rejected_reason: rejected_reason.clone(),
        };
        let config = Arc::new(russh::client::Config::default());

        match russh::client::connect(config, *addr, handler).await {
            Ok(h) => {
                connected = Some(h);
                break;
            }
            Err(e) => {
                // Host key tidak cocok BUKAN masalah per-alamat — itu
                // keputusan keamanan yang sama untuk semua alamat host
                // ini. Mencoba alamat berikutnya di sini sama dengan
                // mengulang penolakan yang sama sambil menyembunyikan
                // alasannya, jadi kita berhenti sekarang.
                if let Some(reason) = rejected_reason.lock().ok().and_then(|mut g| g.take()) {
                    return Err(SshEngineError::HostKeyMismatch(reason));
                }
                last_error = Some(e.to_string());
            }
        }
    }

    let mut handle = match connected {
        Some(h) => h,
        None => {
            let last = last_error.unwrap_or_else(|| "tidak ada detail".into());
            // Satu alamat saja: pesan `Connect` biasa sudah cukup jelas,
            // menyebut "dicoba 1 alamat" malah membingungkan.
            return Err(if addrs.len() == 1 {
                SshEngineError::Connect(last)
            } else {
                SshEngineError::AllAddressesFailed {
                    host: profile.host.clone(),
                    tried: addrs.len(),
                    last,
                }
            });
        }
    };

    // --- Tahap 3: autentikasi.
    report(progress, ConnectStage::Authenticating, Some(profile.username.clone()));
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
    connect_with_progress(profile, secret, host_key_store, None).await
}

/// Seperti `connect`, tapi melaporkan tiap tahap ke `progress` supaya UI
/// bisa menarasikan proses connect (lihat `ConnectStage`).
pub async fn connect_with_progress(
    profile: &HostProfile,
    secret: SecretMaterial,
    host_key_store: Arc<dyn HostKeyStore>,
    progress: Option<&ProgressSink>,
) -> Result<SshSession, SshEngineError> {
    let handle = connect_handle_with_progress(profile, secret, host_key_store, progress).await?;

    report(progress, ConnectStage::OpeningChannel, None);
    let mut channel = handle.channel_open_session().await.map_err(|e| SshEngineError::Channel(e.to_string()))?;

    report(progress, ConnectStage::RequestingPty, None);
    channel
        .request_pty(true, "xterm-256color", 80, 24, 0, 0, &[])
        .await
        .map_err(|e| SshEngineError::Channel(e.to_string()))?;

    report(progress, ConnectStage::StartingShell, None);
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
