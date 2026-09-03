//! Implementasi `russh::client::Handler`.
//!
//! Bagian paling security-sensitive dari crate ini: `check_server_key`
//! adalah satu-satunya garda anti man-in-the-middle. Kebijakannya (lihat
//! juga `HostKeyStore` di `lib.rs`):
//! - Host key belum pernah dikenal -> percaya otomatis & simpan
//!   (trust-on-first-use, sesuai keputusan produk — lihat README).
//! - Sudah dikenal & cocok -> percaya.
//! - Sudah dikenal tapi BEDA -> tolak koneksi (`Ok(false)`); alasannya
//!   ditulis ke `rejected_reason` supaya `connect()` di `lib.rs` bisa
//!   memberi pesan error yang jelas ke UI (bukan cuma "connection
//!   failed" generik dari russh, yang tidak bilang APA sebabnya).

use crate::HostKeyStore;
use russh::keys::PublicKey;
use std::sync::{Arc, Mutex};

pub struct ClientHandler {
    pub host: String,
    pub port: u16,
    pub host_key_store: Arc<dyn HostKeyStore>,
    pub rejected_reason: Arc<Mutex<Option<String>>>,
}

impl russh::client::Handler for ClientHandler {
    type Error = russh::Error;

    async fn check_server_key(&mut self, server_public_key: &PublicKey) -> Result<bool, Self::Error> {
        let key_line = match server_public_key.to_openssh() {
            Ok(line) => line,
            Err(e) => {
                let msg = format!("gagal encode host key: {e}");
                tracing::warn!("{msg}");
                if let Ok(mut guard) = self.rejected_reason.lock() {
                    *guard = Some(msg);
                }
                return Ok(false);
            }
        };

        match self.host_key_store.lookup(&self.host, self.port) {
            Some(stored) if stored == key_line => Ok(true),
            Some(_) => {
                let msg = format!(
                    "Host key untuk {}:{} BERUBAH dari yang tersimpan sebelumnya — kemungkinan \
                     man-in-the-middle, koneksi ditolak.",
                    self.host, self.port
                );
                tracing::warn!("{msg}");
                if let Ok(mut guard) = self.rejected_reason.lock() {
                    *guard = Some(msg);
                }
                Ok(false)
            }
            None => {
                tracing::info!(
                    "host key baru untuk {}:{}, dipercaya otomatis (trust-on-first-use) & disimpan",
                    self.host,
                    self.port
                );
                self.host_key_store.trust(&self.host, self.port, key_line);
                Ok(true)
            }
        }
    }
}
