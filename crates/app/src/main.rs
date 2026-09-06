// File .slint dicompile jadi module Rust oleh build.rs.
slint::include_modules!();

mod app_config;
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

    // Milestone 2b: `state.rs` sudah pakai `VaultBackend` (abstraksi
    // Local/Self-hosted), TAPI startup di sini MASIH SELALU buka mode
    // Local — toggle mode beneran (baca `app_config.rs`, tampilkan
    // layar login Self-hosted) itu Milestone 3, lihat
    // docs/desktop-selfhosted-integration.md.
    let vault = terminus_vault::VaultStore::open_default()?;
    let vault = terminus_vault::VaultBackend::Local(std::sync::Arc::new(std::sync::Mutex::new(vault)));

    let ui = AppWindow::new()?;
    state::wire_callbacks(&ui, vault);

    ui.run()?;
    Ok(())
}
