//! Alur login mode Self-hosted di `VaultDialog` — lihat
//! `docs/desktop-selfhosted-integration.md` bagian 2.5 (refresh token),
//! 4 (resolusi vault) dan 5 (alur UI). Terpisah dari `state.rs` karena
//! ini lifecycle "sebelum unlock" yang cuma ada di mode Self-hosted
//! (bagian 2.2: TIDAK lewat `VaultBackend`).
//!
//! Semua jalur sukses berakhir di `finish_login`: simpan refresh token
//! (terenkripsi, `session_store`) -> pilih vault -> simpan
//! `app_config.json` -> ganti `AppState.vault` jadi `Remote` -> buka UI.

use crate::app_config::{self, AppConfig, SelfHostedConfig, StorageMode};
use crate::state::{close_remote_sessions_for_logout, refresh_hosts_model, refresh_vault_cache, AppState};
use crate::{session_store, AppWindow, VaultModel};
use slint::ComponentHandle;
use std::sync::Arc;
use terminus_vault::{AuthTokens, RemoteVaultClient, VaultBackend, VaultError};

/// Nama vault yang otomatis dibuat kalau user belum jadi anggota vault
/// mana pun (bagian 4 doc — mirror "vault.db baru otomatis dibikin"
/// waktu first-run mode Local).
const DEFAULT_VAULT_NAME: &str = "My Vault";

/// Isi awal `VaultDialog` dari `app_config.json` (tab aktif + prefill
/// URL), lalu coba login diam-diam kalau mode terakhir Self-hosted DAN
/// ada refresh token tersimpan.
pub fn apply_startup_config(ui: &AppWindow, state: &Arc<AppState>, config: &AppConfig) {
    let vm = ui.global::<VaultModel>();
    vm.set_self_hosted_tab(config.mode == StorageMode::SelfHosted);
    let server_url = config.self_hosted.as_ref().map(|c| c.server_url.clone());
    if let Some(url) = &server_url {
        vm.set_server_url(url.into());
    }

    if config.mode != StorageMode::SelfHosted {
        return;
    }
    let Some(server_url) = server_url else { return };
    let refresh_token = match session_store::load_refresh_token() {
        Ok(Some(token)) => token,
        Ok(None) => return,
        Err(e) => {
            // `session.key` hilang/berubah -> token lama tidak bisa
            // didekripsi lagi; user cukup login ulang manual.
            tracing::warn!("gagal baca refresh token tersimpan: {e}");
            return;
        }
    };

    vm.set_busy(true);
    vm.set_status_message("Masuk otomatis...".into());
    let ui_weak = ui.as_weak();
    let state = state.clone();
    tokio::spawn(async move {
        let result = match RemoteVaultClient::refresh_session(&server_url, &refresh_token).await {
            Ok(tokens) => finish_login(&state, &server_url, tokens).await,
            Err(VaultError::SessionExpired(msg)) => {
                // Token sudah dicabut/kedaluwarsa di server — buang,
                // supaya start berikutnya tidak mencoba token mati lagi.
                let _ = tokio::task::spawn_blocking(session_store::clear).await;
                Err(format!("Sesi berakhir, silakan masuk lagi. ({msg})"))
            }
            // Server mati/jaringan putus: token SENGAJA tidak dibuang.
            Err(e) => Err(e.to_string()),
        };
        deliver_result(ui_weak, state, result);
    });
}

pub fn wire_auth_callbacks(ui: &AppWindow, state: &Arc<AppState>) {
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.global::<VaultModel>().on_logout_requested(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            logout(&ui, &state);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.global::<VaultModel>().on_login_requested(move |url, email, password| {
            let request = AuthRequest::Login { email: email.to_string(), password: password.to_string() };
            start_auth(&ui_weak, &state, url.as_str(), request);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.global::<VaultModel>().on_register_requested(move |url, email, password, confirm| {
            let Some(ui) = ui_weak.upgrade() else { return };
            if password != confirm {
                ui.global::<VaultModel>().set_error_message("Konfirmasi password tidak sama.".into());
                return;
            }
            let request = AuthRequest::Register { email: email.to_string(), password: password.to_string() };
            start_auth(&ui_weak, &state, url.as_str(), request);
        });
    }
}

/// Logout eksplisit (tombol di sidebar). Refresh token tersimpan dihapus
/// SINKRON di sini, bukan di task background: kalau app langsung ditutup
/// setelah klik Logout, start berikutnya TIDAK BOLEH masuk otomatis.
/// Pencabutan token di server (`POST /auth/logout`) best-effort di
/// background — gagal (server mati) cuma di-log, token itu tetap tidak
/// bisa dipakai lagi dari mesin ini karena salinannya sudah dihapus.
/// `app_config.json` TIDAK diubah: mode tetap Self-hosted & URL tetap
/// ter-prefill di form login.
fn logout(ui: &AppWindow, state: &Arc<AppState>) {
    let remote = match state.vault() {
        VaultBackend::Remote(client) => Some(client),
        VaultBackend::Local(_) => None,
    };
    // Tandai logout SEBELUM hapus token tersimpan — lihat `logged_out`
    // di `RemoteVaultClient` (race dengan refresh token di background).
    if let Some(client) = &remote {
        client.mark_logged_out();
    }
    if let Err(e) = session_store::clear() {
        tracing::warn!("gagal hapus refresh token tersimpan waktu logout: {e}");
    }
    if let Some(client) = remote {
        tokio::spawn(async move {
            if let Err(e) = client.logout().await {
                tracing::warn!("gagal cabut refresh token di server waktu logout: {e}");
            }
        });
    }
    close_remote_sessions_for_logout(ui, state);

    let vm = ui.global::<VaultModel>();
    vm.set_self_hosted_session(false);
    vm.set_account_email("".into());
    vm.set_self_hosted_tab(true);
    vm.set_register_mode(false);
    vm.set_busy(false);
    vm.set_error_message("".into());
    vm.set_status_message("Kamu sudah logout.".into());
    vm.set_is_unlocked(false);
}

enum AuthRequest {
    Login { email: String, password: String },
    Register { email: String, password: String },
}

fn start_auth(ui_weak: &slint::Weak<AppWindow>, state: &Arc<AppState>, raw_url: &str, request: AuthRequest) {
    let Some(ui) = ui_weak.upgrade() else { return };
    let vm = ui.global::<VaultModel>();
    if vm.get_busy() {
        return; // sudah ada request jalan, abaikan klik dobel
    }
    let server_url = match normalize_server_url(raw_url) {
        Ok(url) => url,
        Err(msg) => {
            vm.set_error_message(msg.into());
            return;
        }
    };
    vm.set_busy(true);
    vm.set_error_message("".into());
    vm.set_status_message("".into());

    let ui_weak = ui_weak.clone();
    let state = state.clone();
    tokio::spawn(async move {
        let tokens = match &request {
            AuthRequest::Login { email, password } => RemoteVaultClient::login(&server_url, email, password).await,
            AuthRequest::Register { email, password } => RemoteVaultClient::register(&server_url, email, password).await,
        };
        let result = match tokens {
            Ok(tokens) => finish_login(&state, &server_url, tokens).await,
            Err(e) => Err(e.to_string()),
        };
        deliver_result(ui_weak, state, result);
    });
}

/// Trim + buang `/` di akhir (semua path API ditempel sebagai
/// `{base}/api/v1/...`). Skema WAJIB eksplisit — menebak `https://`
/// diam-diam bisa bikin user bingung kenapa server `http://` lokalnya
/// tidak bisa dihubungi.
fn normalize_server_url(raw: &str) -> Result<String, String> {
    let url = raw.trim().trim_end_matches('/');
    if url.is_empty() {
        return Err("Server URL wajib diisi.".into());
    }
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err("Server URL harus diawali http:// atau https://".into());
    }
    Ok(url.to_string())
}

/// `Ok` berisi email akun (buat sidebar) — kosong kalau `/auth/me` gagal
/// (tidak menggagalkan login).
async fn finish_login(state: &Arc<AppState>, server_url: &str, tokens: AuthTokens) -> Result<String, String> {
    persist_refresh_token(tokens.refresh_token.clone()).await;

    let previous = tokio::task::spawn_blocking(app_config::load)
        .await
        .map_err(|e| e.to_string())?
        .unwrap_or_default();
    // `vault_id` lama cuma relevan kalau server-nya SAMA.
    let preferred_vault_id = previous
        .self_hosted
        .as_ref()
        .filter(|c| c.server_url == server_url)
        .and_then(|c| c.vault_id.clone());
    let vault_id = resolve_vault_id(server_url, &tokens.access_token, preferred_vault_id.as_deref())
        .await
        .map_err(|e| e.to_string())?;
    let email = RemoteVaultClient::me(server_url, &tokens.access_token).await.unwrap_or_else(|e| {
        tracing::warn!("gagal ambil email akun: {e}");
        String::new()
    });

    let config = AppConfig {
        mode: StorageMode::SelfHosted,
        self_hosted: Some(SelfHostedConfig { server_url: server_url.to_string(), vault_id: Some(vault_id.clone()) }),
    };
    match tokio::task::spawn_blocking(move || app_config::save(&config)).await {
        Ok(Err(e)) => tracing::warn!("gagal simpan app_config.json: {e}"),
        Err(e) => tracing::warn!("task simpan app_config.json panik: {e}"),
        Ok(Ok(())) => {}
    }

    let client = RemoteVaultClient::new(server_url.to_string(), vault_id, tokens.access_token, tokens.refresh_token)
        // Backend me-rotate refresh token tiap `/auth/refresh` (yang
        // lama langsung dicabut) — token baru WAJIB dipersist.
        .with_token_persister(|token| {
            if let Err(e) = session_store::save_refresh_token(&token) {
                tracing::warn!("gagal simpan refresh token hasil rotasi: {e}");
            }
        });
    state.set_vault(VaultBackend::Remote(client));
    refresh_vault_cache(state).await;
    Ok(email)
}

/// Bagian 4 doc. Vault di `app_config.json` dipakai lagi SELAMA user
/// masih anggotanya; kalau tidak, 0 vault -> buat "My Vault", >=1 ->
/// vault pertama.
// TODO(Milestone 5): >1 vault -> tampilkan daftar pilihan sekali waktu
// login, bukan otomatis ambil yang pertama.
async fn resolve_vault_id(server_url: &str, access_token: &str, preferred: Option<&str>) -> Result<String, VaultError> {
    let vaults = RemoteVaultClient::list_vaults(server_url, access_token).await?;
    if let Some(id) = preferred {
        if vaults.iter().any(|v| v.id == id) {
            return Ok(id.to_string());
        }
    }
    match vaults.into_iter().next() {
        Some(first) => Ok(first.id),
        None => Ok(RemoteVaultClient::create_vault(server_url, access_token, DEFAULT_VAULT_NAME).await?.id),
    }
}

async fn persist_refresh_token(token: String) {
    match tokio::task::spawn_blocking(move || session_store::save_refresh_token(&token)).await {
        // Login tetap lanjut — cuma login otomatis di start berikutnya
        // yang tidak akan jalan.
        Ok(Err(e)) => tracing::warn!("gagal simpan refresh token: {e}"),
        Err(e) => tracing::warn!("task simpan refresh token panik: {e}"),
        Ok(Ok(())) => {}
    }
}

fn deliver_result(ui_weak: slint::Weak<AppWindow>, state: Arc<AppState>, result: Result<String, String>) {
    let _ = slint::invoke_from_event_loop(move || {
        let Some(ui) = ui_weak.upgrade() else { return };
        let vm = ui.global::<VaultModel>();
        vm.set_busy(false);
        vm.set_status_message("".into());
        match result {
            Ok(email) => {
                vm.set_error_message("".into());
                vm.set_self_hosted_session(true);
                vm.set_account_email(email.into());
                vm.set_is_unlocked(true);
                refresh_hosts_model(&ui, &state);
            }
            Err(msg) => vm.set_error_message(msg.into()),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::normalize_server_url;

    #[test]
    fn normalize_server_url_buang_slash_akhir_dan_wajib_skema() {
        assert_eq!(normalize_server_url("  https://vault.contoh.com/ ").unwrap(), "https://vault.contoh.com");
        assert_eq!(normalize_server_url("http://localhost:3000").unwrap(), "http://localhost:3000");
        assert!(normalize_server_url("vault.contoh.com").is_err());
        assert!(normalize_server_url("   ").is_err());
    }
}
