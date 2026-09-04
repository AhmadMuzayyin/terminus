//! `terminus-sftp-engine`
//!
//! SFTP dijalankan di atas koneksi SSH yang SAMA (satu `Handle` dari
//! `terminus-ssh-engine::connect_handle`), tapi channel-nya BEDA dari
//! sesi shell interaktif — `request_subsystem("sftp")`, bukan
//! `request_shell` — persis bagaimana Termius/WinSCP juga bekerja: satu
//! tab terminal dan satu file browser bisa berbagi transport SSH yang
//! sama, tapi channel-nya sendiri-sendiri. v1 ini SENGAJA selalu buka
//! koneksi SSH BARU khusus buat SFTP (bukan reuse channel dari tab
//! terminal yang MUNGKIN sudah terbuka buat host yang sama) — lebih
//! sederhana & aman (tidak ada risiko dua fitur berebut channel), reuse
//! channel-terminal-yang-sudah-ada bisa jadi optimisasi nanti.

use terminus_core::HostProfile;
use terminus_ssh_engine::{self as ssh_engine, HostKeyStore, SecretMaterial, SshEngineError};
use russh::client::Handle;
use russh_sftp::client::SftpSession;
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SftpError {
    #[error("koneksi SSH gagal: {0}")]
    Connect(#[from] SshEngineError),

    #[error("gagal membuka subsystem sftp: {0}")]
    OpenFailed(String),

    #[error("operasi sftp gagal: {0}")]
    Io(String),
}

#[derive(Debug, Clone)]
pub struct RemoteEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    /// Detik unix epoch, `None` kalau server tidak kirim mtime.
    pub modified: Option<i64>,
}

/// Satu sesi SFTP yang beneran terkoneksi ke satu host. Nyimpen
/// `Handle<ClientHandler>` (`_handle`) SEKALIGUS `SftpSession` —
/// keduanya WAJIB tetap hidup bareng: `_handle` di-drop = socket
/// ketutup = `SftpSession` ikut mati juga.
pub struct SftpBrowser {
    session: SftpSession,
    _handle: Handle<terminus_ssh_engine::client::ClientHandler>,
}

impl SftpBrowser {
    /// Buka koneksi SSH baru KHUSUS buat SFTP dari sebuah `HostProfile`
    /// tersimpan — kredensial sudah didecrypt caller dari vault, sama
    /// persis pola `terminus_ssh_engine::connect`.
    pub async fn connect(
        profile: &HostProfile,
        secret: SecretMaterial,
        host_key_store: Arc<dyn HostKeyStore>,
    ) -> Result<Self, SftpError> {
        let handle = ssh_engine::connect_handle(profile, secret, host_key_store).await?;
        Self::open_subsystem(handle).await
    }

    async fn open_subsystem(handle: Handle<terminus_ssh_engine::client::ClientHandler>) -> Result<Self, SftpError> {
        let mut channel = handle.channel_open_session().await.map_err(|e| SftpError::OpenFailed(e.to_string()))?;

        channel
            .request_subsystem(true, "sftp")
            .await
            .map_err(|e| SftpError::OpenFailed(e.to_string()))?;

        // Tunggu balasan SSH_MSG_CHANNEL_SUCCESS/FAILURE sebagai bukti
        // subsystem-nya beneran diterima server. BEDA dari `request_
        // shell` di `terminus-ssh-engine::connect` (yang langsung match
        // SEKALI pesan pertama) — di sini SENGAJA di-loop: sebelum
        // Success/Failure beneran datang, server BOLEH kirim
        // `WindowAdjusted` (bookkeeping flow-control biasa, bisa nongol
        // KAPAN SAJA di siklus hidup channel manapun, tidak ada
        // hubungannya dengan status request subsystem) — root cause
        // bug "koneksi terputus sebelum terhubung" yang dilaporkan
        // user: pesan `WindowAdjusted` itu ke-anggap "bukan Success/
        // Failure jadi pasti terputus" oleh match SEKALI-tembak yang
        // lama, padahal koneksinya baik-baik saja. Sekarang pesan
        // bookkeeping kayak gitu di-skip, TERUS nunggu sampai beneran
        // dapat Success/Failure/tanda channel mati.
        loop {
            match channel.wait().await {
                Some(russh::ChannelMsg::Success) => break,
                Some(russh::ChannelMsg::Failure) => {
                    return Err(SftpError::OpenFailed(
                        "server menolak subsystem sftp (mungkin tidak didukung server ini)".into(),
                    ));
                }
                Some(russh::ChannelMsg::WindowAdjusted { .. }) => continue,
                Some(russh::ChannelMsg::Eof) | Some(russh::ChannelMsg::Close) | None => {
                    return Err(SftpError::OpenFailed("koneksi terputus sebelum subsystem sftp siap".into()));
                }
                // Pesan bookkeeping lain yang mungkin ikut nyempil
                // (jarang, tapi tidak fatal) — abaikan, terus tunggu.
                Some(_) => continue,
            }
        }

        let stream = channel.into_stream();
        let session = SftpSession::new(stream).await.map_err(|e| SftpError::OpenFailed(e.to_string()))?;

        Ok(Self { session, _handle: handle })
    }

    /// Path home direktori remote ("." di-canonicalize server) —
    /// dipakai sebagai titik awal browsing begitu SFTP baru terkoneksi.
    pub async fn home_dir(&self) -> Result<String, SftpError> {
        self.session.canonicalize(".").await.map_err(|e| SftpError::Io(e.to_string()))
    }

    /// Baca SELURUH isi satu file remote jadi byte — dipakai buat
    /// "download" (klik file di panel Remote, tombol "← Copy", lihat
    /// `crates/app/src/state.rs::on_sftp_copy_to_local_requested`).
    /// v1 SENGAJA cuma file tunggal (bukan folder rekursif) — transfer
    /// folder butuh logic jauh lebih rumit (list rekursif + retry
    /// per-file), di luar scope putaran ini.
    pub async fn download(&self, remote_path: &str) -> Result<Vec<u8>, SftpError> {
        self.session.read(remote_path).await.map_err(|e| SftpError::Io(e.to_string()))
    }

    /// Tulis byte ke satu file remote (bikin baru/timpa) — dipakai buat
    /// "upload" (drag & drop dari panel Local ke Remote).
    ///
    /// SENGAJA BUKAN `self.session.write(...)` bawaan `russh_sftp` —
    /// itu convenience method di library-nya sendiri buka file cuma
    /// pakai flag `OpenFlags::WRITE` doang (lihat source-nya,
    /// `russh-sftp-2.4.0/src/client/session.rs`), TANPA `CREATE` —
    /// artinya kalau file itu BELUM ADA di remote (kasus PALING umum
    /// waktu upload: bikin file baru), server SFTP menolak dengan "no
    /// such file", upload selalu gagal. Ini bug nyata di upstream,
    /// bukan di kode kita — root cause laporan user "upload dari lokal
    /// ke server belum bisa, download jalan" (download cuma BACA file
    /// yang SUDAH ADA, tidak kena masalah CREATE sama sekali). Fix:
    /// pakai `session.create()` (juga disediakan `russh_sftp`,
    /// convenience method LAIN yang benar — buka dengan
    /// `CREATE | TRUNCATE | WRITE`, persis semantik "bikin baru/timpa"
    /// yang kita mau) lalu `write_all` manual.
    pub async fn upload(&self, remote_path: &str, data: &[u8]) -> Result<(), SftpError> {
        use tokio::io::AsyncWriteExt;
        let mut file = self.session.create(remote_path).await.map_err(|e| SftpError::Io(e.to_string()))?;
        file.write_all(data).await.map_err(|e| SftpError::Io(e.to_string()))?;
        Ok(())
    }

    /// Ganti nama/pindahkan satu entry remote (file ATAU folder) —
    /// dipakai menu "Rename" (lihat page-sftp.slint).
    pub async fn rename(&self, old_path: &str, new_path: &str) -> Result<(), SftpError> {
        self.session.rename(old_path, new_path).await.map_err(|e| SftpError::Io(e.to_string()))
    }

    /// Hapus satu FILE remote (bukan folder — "Delete" di menu cuma
    /// beroperasi ke file yang lagi dicentang, folder tidak bisa
    /// dicentang, lihat `FileEntry.checked` & wiring-nya di
    /// state.rs/page-sftp.slint).
    pub async fn remove_file(&self, remote_path: &str) -> Result<(), SftpError> {
        self.session.remove_file(remote_path).await.map_err(|e| SftpError::Io(e.to_string()))
    }

    /// Bikin folder baru remote — dipakai menu "New Folder".
    pub async fn create_dir(&self, remote_path: &str) -> Result<(), SftpError> {
        self.session.create_dir(remote_path).await.map_err(|e| SftpError::Io(e.to_string()))
    }

    /// List isi satu direktori remote. `.`/`..` sudah difilter otomatis
    /// oleh `russh_sftp`. Diurut: folder dulu (alfabetis), baru file
    /// (alfabetis) — konsisten sama gaya file manager pada umumnya.
    pub async fn list_dir(&self, path: &str) -> Result<Vec<RemoteEntry>, SftpError> {
        let read_dir = self.session.read_dir(path).await.map_err(|e| SftpError::Io(e.to_string()))?;

        let mut entries: Vec<RemoteEntry> = read_dir
            .map(|entry| {
                let meta = entry.metadata();
                RemoteEntry {
                    name: entry.file_name(),
                    is_dir: entry.file_type().is_dir(),
                    size: meta.size.unwrap_or(0),
                    modified: meta.mtime.map(|t| t as i64),
                }
            })
            .collect();

        entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        });

        Ok(entries)
    }
}
