//! Adapter yang menjembatani `terminus_ssh_engine::HostKeyStore` (trait
//! yang didefinisikan di `ssh-engine`, supaya crate itu tidak perlu
//! depend langsung ke `vault` — lihat aturan arah dependency di
//! `docs/architecture.md`) ke `terminus_vault::VaultStore` yang
//! sebenarnya menyimpan datanya (tabel `known_hosts`).
//!
//! `vault` = `None` di mode Self-hosted (lihat
//! `docs/desktop-selfhosted-integration.md` bagian 2.7): `lookup` selalu
//! `None` dan `trust` no-op, jadi ssh-engine memperlakukan TIAP koneksi
//! sebagai first-use dan menerima host key-nya tanpa diingat.

use terminus_ssh_engine::HostKeyStore;
use terminus_vault::VaultStore;
use std::sync::{Arc, Mutex};

pub struct AppHostKeyStore {
    vault: Option<Arc<Mutex<VaultStore>>>,
}

impl AppHostKeyStore {
    pub fn new(vault: Option<Arc<Mutex<VaultStore>>>) -> Self {
        Self { vault }
    }
}

impl HostKeyStore for AppHostKeyStore {
    fn lookup(&self, host: &str, port: u16) -> Option<String> {
        self.vault.as_ref()?.lock().ok()?.check_host_key(host, port).ok().flatten()
    }

    fn trust(&self, host: &str, port: u16, key_line: String) {
        let Some(vault) = &self.vault else { return };
        if let Ok(mut vault) = vault.lock() {
            if let Err(e) = vault.trust_host_key(host, port, &key_line) {
                tracing::warn!("gagal simpan host key untuk {host}:{port}: {e}");
            }
        }
    }
}
