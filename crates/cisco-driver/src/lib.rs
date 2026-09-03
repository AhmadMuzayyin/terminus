//! `terminus-cisco-driver`
//!
//! SSH ke Cisco IOS/IOS-XE itu secara protokol ya SSH biasa — tapi
//! perilaku CLI-nya beda dari shell Linux, jadi butuh layer khusus di
//! atas `terminus-ssh-engine`:
//!
//! - **Paging**: output panjang kepotong "--More--", perlu auto-kirim
//!   spasi, atau lebih baik langsung `terminal length 0` saat connect.
//! - **Mode**: user exec (`>`) vs privileged exec (`#`) vs config mode
//!   (`(config)#`) — perlu deteksi prompt buat tahu device lagi di mode apa.
//! - **Enable password**: kadang perlu kirim `enable` + password terpisah
//!   dari auth SSH awal.

use terminus_ssh_engine::SshSession;
use thiserror::Error;

pub mod prompt;

#[derive(Debug, Error)]
pub enum CiscoDriverError {
    #[error("gagal masuk privileged mode: {0}")]
    EnableFailed(String),

    #[error("prompt tidak dikenali, kemungkinan bukan device Cisco IOS")]
    UnrecognizedPrompt,
}

/// Mode CLI Cisco saat ini, dideteksi dari pola prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CiscoMode {
    UserExec,   // Router>
    Privileged, // Router#
    Config,     // Router(config)#
}

pub struct CiscoSession {
    // TODO: bungkus SshSession + state mode saat ini
    mode: CiscoMode,
}

impl CiscoSession {
    /// Bootstrap sesi Cisco: connect SSH lalu langsung kirim
    /// `terminal length 0` supaya tidak ada paging "--More--".
    pub async fn bootstrap(_ssh: SshSession) -> Result<Self, CiscoDriverError> {
        // TODO: kirim "terminal length 0\n", tunggu prompt balik
        Ok(Self { mode: CiscoMode::UserExec })
    }

    pub async fn enable(&mut self, _password: &str) -> Result<(), CiscoDriverError> {
        // TODO: kirim "enable\n", deteksi prompt "Password:", kirim password
        self.mode = CiscoMode::Privileged;
        Ok(())
    }

    pub fn current_mode(&self) -> CiscoMode {
        self.mode
    }
}
