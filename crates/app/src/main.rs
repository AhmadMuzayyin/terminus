// File .slint dicompile jadi module Rust oleh build.rs.
slint::include_modules!();

mod app_config;
mod auth_flow;
mod console;
mod host_key_store;
mod session_store;
mod state;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    // Slint butuh jalan di main thread (event loop native OS).
    // Kerja berat/async (SSH) didelegasikan ke tokio runtime terpisah
    // di background, komunikasi balik ke UI lewat
    // slint::invoke_from_event_loop / Weak<AppWindow> — lihat state.rs.
    let tokio_rt = tokio::runtime::Runtime::new()?;
    let _guard = tokio_rt.enter();

    // Config rusak tidak boleh bikin app gagal start — jatuh ke default
    // (mode Local), user tinggal pilih tab lagi di `VaultDialog`.
    let config = app_config::load().unwrap_or_else(|e| {
        tracing::warn!("app_config.json tidak terbaca, pakai default Local: {e}");
        app_config::AppConfig::default()
    });

    // `vault.db` lokal SELALU dibuka (tab Local tetap bisa dipakai walau
    // mode terakhir Self-hosted). Backend `Remote` baru dipasang setelah
    // login sukses — lihat `auth_flow` & docs/desktop-selfhosted-integration.md.
    let vault = terminus_vault::VaultStore::open_default()?;
    let vault = terminus_vault::VaultBackend::Local(std::sync::Arc::new(std::sync::Mutex::new(vault)));

    let ui = AppWindow::new()?;
    let state = state::wire_callbacks(&ui, vault);
    auth_flow::wire_auth_callbacks(&ui, &state);
    auth_flow::apply_startup_config(&ui, &state, &config);

    ui.run()?;
    Ok(())
}
