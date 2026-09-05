//! Konfigurasi app-level yang menentukan mode penyimpanan aktif
//! (Local vs Self-hosted) SEBELUM ada vault/koneksi apa pun dibuka —
//! lihat `docs/desktop-selfhosted-integration.md` bagian 3.
//!
//! Semua field di sini SENGAJA non-rahasia (mode, URL server, id
//! vault) sehingga file ini boleh plaintext. Refresh token (rahasia)
//! TIDAK ADA di sini — itu di `session_store` (terenkripsi terpisah).

// `dead_code` sementara: modul ini belum dipanggil dari `main.rs`/
// `state.rs` (itu Milestone 3, wiring layar login/mode di
// `docs/desktop-selfhosted-integration.md`) — dihapus waktu wiring
// itu dikerjakan.
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageMode {
    Local,
    SelfHosted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelfHostedConfig {
    pub server_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vault_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub mode: StorageMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub self_hosted: Option<SelfHostedConfig>,
}

impl Default for AppConfig {
    /// Default SELALU `Local` — instalasi lama yang belum pernah
    /// punya `app_config.json` (upgrade dari versi sebelum fitur ini
    /// ada) TIDAK BOLEH berubah perilaku begitu saja.
    fn default() -> Self {
        Self { mode: StorageMode::Local, self_hosted: None }
    }
}

fn config_path() -> anyhow::Result<PathBuf> {
    let dirs = directories::ProjectDirs::from("com", "terminus", "terminus")
        .ok_or_else(|| anyhow::anyhow!("tidak bisa resolve config dir"))?;
    Ok(dirs.config_dir().join("app_config.json"))
}

/// Baca `app_config.json`. File belum ada (instalasi lama ATAU
/// benar-benar pertama kali) -> `AppConfig::default()` (mode Local),
/// BUKAN error.
pub fn load() -> anyhow::Result<AppConfig> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(AppConfig::default());
    }
    let raw = std::fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&raw)?)
}

pub fn save(config: &AppConfig) -> anyhow::Result<()> {
    let path = config_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let raw = serde_json::to_string_pretty(config)?;
    std::fs::write(&path, raw)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_selalu_mode_local() {
        let config = AppConfig::default();
        assert_eq!(config.mode, StorageMode::Local);
        assert!(config.self_hosted.is_none());
    }

    #[test]
    fn roundtrip_json_self_hosted() {
        let config = AppConfig {
            mode: StorageMode::SelfHosted,
            self_hosted: Some(SelfHostedConfig {
                server_url: "https://vault.contoh.com".into(),
                vault_id: Some("11111111-1111-1111-1111-111111111111".into()),
            }),
        };
        let raw = serde_json::to_string(&config).unwrap();
        let parsed: AppConfig = serde_json::from_str(&raw).unwrap();
        assert_eq!(parsed.mode, StorageMode::SelfHosted);
        assert_eq!(parsed.self_hosted.unwrap().server_url, "https://vault.contoh.com");
    }
}
