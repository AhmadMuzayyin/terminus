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

use std::sync::{Arc, Mutex};

use terminus_core::{HostGroup, HostProfile, Identity};
use uuid::Uuid;

use thiserror::Error;

pub mod crypto;
pub mod remote;
pub mod store;

pub use remote::RemoteVaultClient;
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

    /// Dipetakan dari respons HTTP non-2xx backend (mis. `{ "error":
    /// "..." }`, lihat `errorHandler.ts` di `backend/`) atau error
    /// jaringan (timeout/connection refused/dst) waktu mode
    /// Self-hosted aktif — lihat
    /// `docs/desktop-selfhosted-integration.md` bagian 2.4. TIDAK
    /// PERNAH muncul di mode Local (murni file lokal, tidak ada
    /// jaringan).
    #[error("server error: {0}")]
    Remote(String),
}

/// Satu titik masuk yang dipakai `crates/app/src/state.rs` TANPA perlu
/// tahu mode aktif (Local/Self-hosted) — lihat
/// `docs/desktop-selfhosted-integration.md` bagian 2 buat rasional
/// lengkap. `#[derive(Clone)]` MURAH (isinya `Arc` semua) — TIDAK ADA
/// `Arc<Mutex<VaultBackend>>` di luar, sinkronisasi cukup di dalam
/// tiap varian (`Arc<Mutex<VaultStore>>` buat Local, internal ke
/// `RemoteVaultClient` buat Remote).
///
/// method di sini `async fn` — Local membungkus panggilan sync
/// `VaultStore` pakai `spawn_blocking` DI SINI (satu tempat), Remote
/// pakai `reqwest::Client` async native langsung (lihat `remote.rs`
/// bagian kenapa TIDAK pakai `reqwest::blocking`).
///
/// method `is_initialized`/`initialize`/`unlock`/`lock`/`is_unlocked`
/// SENGAJA TIDAK ADA di sini (beda total flow-nya per mode, ditangani
/// terpisah di layer startup app — lihat bagian 2.2/5 doc) —
/// `check_host_key`/`trust_host_key` juga SENGAJA TIDAK ADA (lihat
/// bagian 2.7 doc, `local_store()` di bawah dipakai buat itu).
#[derive(Clone)]
pub enum VaultBackend {
    Local(Arc<Mutex<VaultStore>>),
    Remote(RemoteVaultClient),
}

/// Dilempar kalau `tokio::task::spawn_blocking` panik (bukan error
/// biasa dari body closure-nya) — praktis tidak pernah kejadian
/// kecuali bug/OOM, tapi tetap harus dipetakan ke `VaultError` supaya
/// signature method tetap `Result<_, VaultError>` di kedua varian.
fn blocking_task_panicked(e: tokio::task::JoinError) -> VaultError {
    VaultError::Database(format!("task background panik: {e}"))
}

impl VaultBackend {
    /// `Some` cuma buat varian `Local` — dipakai `main.rs` bikin
    /// `AppHostKeyStore` (lihat bagian 2.7 doc; `None` di mode
    /// Self-hosted, `AppHostKeyStore` tangani itu dengan tidak
    /// menyimpan/mengingat host key sama sekali sampai ada
    /// `known_hosts` store yang always-local, milestone terpisah).
    pub fn local_store(&self) -> Option<Arc<Mutex<VaultStore>>> {
        match self {
            Self::Local(store) => Some(Arc::clone(store)),
            Self::Remote(_) => None,
        }
    }

    pub async fn list_ungrouped_profiles(&self) -> Result<Vec<HostProfile>, VaultError> {
        match self {
            Self::Local(store) => {
                let store = Arc::clone(store);
                tokio::task::spawn_blocking(move || store.lock().unwrap().list_ungrouped_profiles())
                    .await
                    .map_err(blocking_task_panicked)?
            }
            Self::Remote(client) => client.list_ungrouped_profiles().await,
        }
    }

    pub async fn list_profiles_in_group(&self, group_id: Uuid) -> Result<Vec<HostProfile>, VaultError> {
        match self {
            Self::Local(store) => {
                let store = Arc::clone(store);
                tokio::task::spawn_blocking(move || store.lock().unwrap().list_profiles_in_group(group_id))
                    .await
                    .map_err(blocking_task_panicked)?
            }
            Self::Remote(client) => client.list_profiles_in_group(group_id).await,
        }
    }

    pub async fn list_all_profiles(&self) -> Result<Vec<HostProfile>, VaultError> {
        match self {
            Self::Local(store) => {
                let store = Arc::clone(store);
                tokio::task::spawn_blocking(move || store.lock().unwrap().list_all_profiles())
                    .await
                    .map_err(blocking_task_panicked)?
            }
            Self::Remote(client) => client.list_all_profiles().await,
        }
    }

    pub async fn save_profile(&self, profile: HostProfile) -> Result<(), VaultError> {
        match self {
            Self::Local(store) => {
                let store = Arc::clone(store);
                tokio::task::spawn_blocking(move || store.lock().unwrap().save_profile(&profile))
                    .await
                    .map_err(blocking_task_panicked)?
            }
            Self::Remote(client) => client.save_profile(&profile).await,
        }
    }

    pub async fn delete_profile(&self, id: Uuid) -> Result<(), VaultError> {
        match self {
            Self::Local(store) => {
                let store = Arc::clone(store);
                tokio::task::spawn_blocking(move || store.lock().unwrap().delete_profile(id))
                    .await
                    .map_err(blocking_task_panicked)?
            }
            Self::Remote(client) => client.delete_profile(id).await,
        }
    }

    pub async fn list_groups(&self) -> Result<Vec<HostGroup>, VaultError> {
        match self {
            Self::Local(store) => {
                let store = Arc::clone(store);
                tokio::task::spawn_blocking(move || store.lock().unwrap().list_groups())
                    .await
                    .map_err(blocking_task_panicked)?
            }
            Self::Remote(client) => client.list_groups().await,
        }
    }

    pub async fn save_group(&self, group: HostGroup) -> Result<(), VaultError> {
        match self {
            Self::Local(store) => {
                let store = Arc::clone(store);
                tokio::task::spawn_blocking(move || store.lock().unwrap().save_group(&group))
                    .await
                    .map_err(blocking_task_panicked)?
            }
            Self::Remote(client) => client.save_group(&group).await,
        }
    }

    pub async fn delete_group(&self, id: Uuid) -> Result<(), VaultError> {
        match self {
            Self::Local(store) => {
                let store = Arc::clone(store);
                tokio::task::spawn_blocking(move || store.lock().unwrap().delete_group(id))
                    .await
                    .map_err(blocking_task_panicked)?
            }
            Self::Remote(client) => client.delete_group(id).await,
        }
    }

    pub async fn store_secret(&self, credential_id: Uuid, plaintext: Vec<u8>) -> Result<(), VaultError> {
        match self {
            Self::Local(store) => {
                let store = Arc::clone(store);
                tokio::task::spawn_blocking(move || store.lock().unwrap().store_secret(credential_id, &plaintext))
                    .await
                    .map_err(blocking_task_panicked)?
            }
            Self::Remote(client) => client.store_secret(credential_id, &plaintext).await,
        }
    }

    pub async fn read_secret(&self, credential_id: Uuid) -> Result<Vec<u8>, VaultError> {
        match self {
            Self::Local(store) => {
                let store = Arc::clone(store);
                tokio::task::spawn_blocking(move || store.lock().unwrap().read_secret(credential_id))
                    .await
                    .map_err(blocking_task_panicked)?
            }
            Self::Remote(client) => client.read_secret(credential_id).await,
        }
    }

    pub async fn delete_secret(&self, credential_id: Uuid) -> Result<(), VaultError> {
        match self {
            Self::Local(store) => {
                let store = Arc::clone(store);
                tokio::task::spawn_blocking(move || store.lock().unwrap().delete_secret(credential_id))
                    .await
                    .map_err(blocking_task_panicked)?
            }
            Self::Remote(client) => client.delete_secret(credential_id).await,
        }
    }

    pub async fn list_identities(&self) -> Result<Vec<Identity>, VaultError> {
        match self {
            Self::Local(store) => {
                let store = Arc::clone(store);
                tokio::task::spawn_blocking(move || store.lock().unwrap().list_identities())
                    .await
                    .map_err(blocking_task_panicked)?
            }
            Self::Remote(client) => client.list_identities().await,
        }
    }

    pub async fn save_identity(&self, identity: Identity) -> Result<(), VaultError> {
        match self {
            Self::Local(store) => {
                let store = Arc::clone(store);
                tokio::task::spawn_blocking(move || store.lock().unwrap().save_identity(&identity))
                    .await
                    .map_err(blocking_task_panicked)?
            }
            Self::Remote(client) => client.save_identity(&identity).await,
        }
    }

    pub async fn delete_identity(&self, id: Uuid) -> Result<(), VaultError> {
        match self {
            Self::Local(store) => {
                let store = Arc::clone(store);
                tokio::task::spawn_blocking(move || store.lock().unwrap().delete_identity(id))
                    .await
                    .map_err(blocking_task_panicked)?
            }
            Self::Remote(client) => client.delete_identity(id).await,
        }
    }
}
