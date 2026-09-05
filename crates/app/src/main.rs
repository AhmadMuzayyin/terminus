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

    let vault = terminus_vault::VaultStore::open_default()?;

    let ui = AppWindow::new()?;
    state::wire_callbacks(&ui, vault);

    ui.run()?;
    Ok(())
}
