//! Lapisan penghubung antara UI thread (Slint) dan tokio runtime tempat
//! semua kerja SSH/vault berjalan. Polanya:
//!
//! 1. SETIAP operasi yang menyentuh vault (SQLite lokal, cepat) ATAU
//!    jaringan (SSH) di-`tokio::spawn` — kerja yang genuinely CPU/IO
//!    (Argon2id derive key, baca/tulis SQLite) dibungkus lagi di dalam
//!    `tokio::task::spawn_blocking` supaya tidak numpang di worker
//!    thread async biasa. Hasilnya dikembalikan ke UI thread lewat
//!    `slint::invoke_from_event_loop`. Ini SENGAJA dipakai bahkan untuk
//!    operasi yang "cepat" (create/save/delete host, dsb) — bukan cuma
//!    SSH — supaya state loading (`HostsModel.creating-host`, dst,
//!    lihat `ui/models.slint`) BENERAN kepakai: Slint cuma repaint
//!    waktu callback Rust sudah return & kontrol balik ke event loop,
//!    jadi set `busy=true` lalu langsung kerja sinkron di callback yang
//!    SAMA tidak akan pernah kelihatan sebagai frame loading — harus
//!    ada jeda thread-hop sungguhan biar event loop sempat repaint.
//! 2. Argon2id (unlock/buat master password) SENGAJA lambat (bagian
//!    dari desain keamanannya) — paling kelihatan butuh loading state
//!    di antara semuanya.

use crate::host_key_store::AppHostKeyStore;
use crate::{
    console, AppWindow, ConnectingModel, ConsoleModel, FileEntry, GroupItem, HostItem, HostsModel, NewHostForm,
    SftpModel, TermRow, TerminalTab, TerminalTabsModel, VaultModel,
};
use terminus_core::{AuthMethod, ConnectionKind, HostGroup, HostProfile};
use terminus_serial_engine::{SerialOutputEvent, SerialPortEntry, SerialSession};
use terminus_sftp_engine::{RemoteEntry, SftpBrowser, SftpError};
use terminus_ssh_engine::{HostKeyStore, SecretMaterial, SshOutputEvent, SshSession};
use terminus_term_emulator::TerminalInstance;
use terminus_vault::VaultStore;
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::sync::Mutex as TokioMutex;
use uuid::Uuid;

/// Metadata satu tab terminal SSH yang lagi terbuka — dipisah dari
/// `sessions` (yang isinya `SshSession` beneran) supaya bisa dibaca
/// sinkron di main thread waktu membangun ulang `TerminalTabsModel`.
/// `Vec`, bukan `HashMap`, supaya urutan tab (insertion order) stabil
/// buat tab bar.
struct TabMeta {
    id: Uuid,
    label: String,
    address: String,
}

/// State yang dipegang bersama sepanjang hidup aplikasi. Semua field
/// dibungkus `Arc` supaya bisa di-`clone()` murah ke tiap closure
/// callback dan ke task tokio yang di-spawn.
pub struct AppState {
    vault: Arc<Mutex<VaultStore>>,
    /// Sesi SSH yang lagi aktif, key-nya `HostProfile::id`. Satu entry
    /// = satu tab terbuka (lihat `tab_meta` di bawah, selalu sinkron
    /// dengan ini — sesi ada di sini KALAU DAN HANYA KALAU tab-nya ada
    /// di `tab_meta`).
    sessions: Arc<TokioMutex<HashMap<Uuid, SshSession>>>,
    /// Status koneksi per host ("offline"/"connecting"/"online"/"failed"),
    /// dipisah dari `sessions` supaya bisa dibaca sinkron di main thread
    /// waktu membangun ulang `HostsModel` tanpa perlu lock async.
    statuses: Arc<Mutex<HashMap<Uuid, String>>>,
    /// Tab mana yang lagi DITAMPILKAN (`TerminalTabsModel.rows`). Tab
    /// lain yang masih terbuka di background tetap hidup, cuma tidak
    /// mendorong update ke UI selama bukan ini. `None` = tidak ada tab
    /// yang lagi ditampilkan (mis. semua tab ketutup).
    active_terminal: Arc<Mutex<Option<Uuid>>>,
    /// Daftar tab yang lagi terbuka, urut sesuai waktu connect.
    tab_meta: Arc<Mutex<Vec<TabMeta>>>,
    /// Snapshot rows TERAKHIR per tab (di-update tiap ada output baru,
    /// termasuk buat tab yang lagi TIDAK ditampilkan) — dipakai supaya
    /// pindah tab langsung nampilin isi terakhirnya, bukan blank
    /// sampai ada byte baru datang.
    tab_cache: Arc<Mutex<HashMap<Uuid, Vec<console::PlainRow>>>>,
    /// `TerminalInstance` (parser VTE) per tab — DULU cuma variabel
    /// lokal di dalam `spawn_terminal_reader`, sekarang di sini supaya
    /// bisa di-snapshot ULANG dari LUAR task pembaca-nya waktu user
    /// ganti color theme (lihat `on_terminal_theme_changed`) tanpa
    /// perlu nunggu byte baru dari SSH buat re-render tampilan yang
    /// sudah ada.
    terminals: Arc<Mutex<HashMap<Uuid, TerminalInstance>>>,
    /// Color theme terminal yang lagi aktif — GLOBAL buat semua tab
    /// (bukan per-host), diubah lewat panel pengaturan (⚙ di halaman
    /// Terminal). Dibaca ULANG tiap kali render (bukan disimpan di
    /// `TerminalInstance`) — lihat `TerminalInstance::snapshot`.
    terminal_theme: Arc<Mutex<terminus_term_emulator::palette::Palette>>,
    /// Id host/grup yang lagi dicentang buat bulk delete (checkbox di
    /// tiap kartu, lihat `HostsModel.host-selection-toggled`/`group-
    /// selection-toggled` di models.slint). Dipegang Rust (bukan
    /// property Slint per-item) supaya toggle-nya gampang: `HashSet`
    /// tinggal insert/remove, lalu `refresh_hosts_model` baca ulang ke
    /// field `checked` tiap `HostItem`/`GroupItem`. Direset kosong
    /// setelah bulk delete sukses.
    selected_hosts: Arc<Mutex<std::collections::HashSet<Uuid>>>,
    selected_groups: Arc<Mutex<std::collections::HashSet<Uuid>>>,
    /// Sesi SFTP yang lagi aktif — SATU SAJA (bukan `HashMap` kayak
    /// `sessions` punya terminal, yang bisa banyak tab sekaligus).
    /// Halaman SFTP v1 murni satu two-pane browser linear: connect ->
    /// browse -> disconnect -> balik ke chooser, TIDAK ada konsep
    /// "banyak sesi SFTP sekaligus" kayak tab terminal. `TokioMutex`
    /// (bukan `std::sync::Mutex`) karena `SftpBrowser::list_dir` itu
    /// `async` — dikunci LINTAS `.await`, `std::sync::Mutex` bakal
    /// nge-block worker thread tokio.
    sftp: Arc<TokioMutex<Option<SftpBrowser>>>,
    /// Path yang lagi ditampilkan panel Local — path FILESYSTEM lokal
    /// beneran (`std::fs`), independen dari `sftp` (tetap ada nilainya
    /// walau belum connect, biar begitu connect langsung ada titik
    /// awal buat di-list).
    sftp_local_path: Arc<Mutex<std::path::PathBuf>>,
    /// Path yang lagi ditampilkan panel Remote — string POSIX (SFTP
    /// SELALU pakai "/" apa pun OS servernya), diisi dari
    /// `SftpBrowser::home_dir()` waktu baru connect.
    sftp_remote_path: Arc<Mutex<String>>,
    /// Nama entry (FILE saja, folder tidak bisa dicentang) yang lagi
    /// dicentang per panel — buat "Select All" + bulk "Delete" di menu
    /// select option. Direset kosong tiap pindah direktori (lihat
    /// `refresh_sftp_panes`) — mencentang di satu folder lalu pindah
    /// folder TIDAK boleh "membawa" centangan ke folder baru.
    sftp_local_selected: Arc<Mutex<std::collections::HashSet<String>>>,
    sftp_remote_selected: Arc<Mutex<std::collections::HashSet<String>>>,
    /// Toggle "Show Hidden Files" per panel — disimpan di SINI (bukan
    /// cuma properti Slint) karena `refresh_sftp_panes` jalan di
    /// background thread (`tokio::spawn`), TIDAK BOLEH sentuh Slint
    /// langsung dari situ (lihat catatan yang sama di komentar
    /// `sftp_local_path`).
    sftp_local_show_hidden: Arc<Mutex<bool>>,
    sftp_remote_show_hidden: Arc<Mutex<bool>>,
    /// Sesi Console (serial) yang lagi aktif — SATU SAJA, sama alasan
    /// dengan `sftp` di atas (port serial fisik memang eksklusif,
    /// tidak ada gunanya pura-pura banyak sesi sekaligus).
    console_session: Arc<TokioMutex<Option<SerialSession>>>,
    /// Parser VTE buat sesi Console — terpisah dari `terminals` (yang
    /// per-host, buat tab SSH) karena Console bukan konsep "tab", cuma
    /// satu grid aktif dalam satu waktu.
    console_terminal: Arc<Mutex<Option<TerminalInstance>>>,
    /// Hasil scan port serial TERAKHIR (`terminus_serial_engine::
    /// list_ports`) — Slint cuma pegang LABEL siap-tampil
    /// (`ConsoleModel.available-ports`), path asli disimpan di sini,
    /// sejajar index, biar `on_console_connect_requested` bisa
    /// nerjemahin `selected-port-index` balik ke path yang beneran
    /// dibuka.
    console_ports: Arc<Mutex<Vec<SerialPortEntry>>>,
    /// Task connect (SSH/SFTP/Console) yang LAGI jalan — dipegang biar
    /// tombol "Batal" di layar `ConnectingModel` (lihat `models.slint`)
    /// bisa benar-benar MENGHENTIKAN proses connect-nya
    /// (`AbortHandle::abort()`), bukan cuma menyembunyikan overlay
    /// (kalau cuma disembunyikan, task-nya tetap jalan di background
    /// lalu "muncul sendiri" belakangan begitu berhasil/gagal —
    /// membingungkan). `Option<Uuid>` = host id kalau ini SSH terminal
    /// connect (perlu di-reset statusnya balik ke "offline" waktu
    /// dibatalkan, lihat `set_status`); `None` buat SFTP/Console (tidak
    /// ada status per-host yang perlu di-reset). SATU slot saja cukup
    /// — layar Connecting SENGAJA cuma bisa nampilin satu proses
    /// connect dalam satu waktu (lihat `ConnectingModel`).
    active_connect: Arc<Mutex<Option<(Option<Uuid>, tokio::task::AbortHandle)>>>,
}

/// Dipanggil sekali dari `main.rs` setelah `AppWindow::new()`. Nyambungin
/// semua callback UI (`HostsModel`/`VaultModel`, lihat `ui/models.slint`)
/// ke `terminus-vault`/`terminus-ssh-engine`. Return value (`Arc<AppState>`)
/// biasanya diabaikan caller (semua closure callback sudah pegang
/// clone-nya sendiri buat tetap hidup) — dipakai test buat inspeksi
/// vault langsung tanpa lewat Slint (mis. bandingkan `credential_id`).
pub fn wire_callbacks(ui: &AppWindow, vault: VaultStore) -> Arc<AppState> {
    let state = Arc::new(AppState {
        vault: Arc::new(Mutex::new(vault)),
        sessions: Arc::new(TokioMutex::new(HashMap::new())),
        statuses: Arc::new(Mutex::new(HashMap::new())),
        active_terminal: Arc::new(Mutex::new(None)),
        tab_meta: Arc::new(Mutex::new(Vec::new())),
        tab_cache: Arc::new(Mutex::new(HashMap::new())),
        terminals: Arc::new(Mutex::new(HashMap::new())),
        terminal_theme: Arc::new(Mutex::new(terminus_term_emulator::palette::terminus_dark())),
        selected_hosts: Arc::new(Mutex::new(std::collections::HashSet::new())),
        selected_groups: Arc::new(Mutex::new(std::collections::HashSet::new())),
        sftp: Arc::new(TokioMutex::new(None)),
        sftp_local_path: Arc::new(Mutex::new(
            std::env::var("HOME").map(std::path::PathBuf::from).unwrap_or_else(|_| std::path::PathBuf::from("/")),
        )),
        sftp_remote_path: Arc::new(Mutex::new("/".to_string())),
        sftp_local_selected: Arc::new(Mutex::new(std::collections::HashSet::new())),
        sftp_remote_selected: Arc::new(Mutex::new(std::collections::HashSet::new())),
        sftp_local_show_hidden: Arc::new(Mutex::new(false)),
        sftp_remote_show_hidden: Arc::new(Mutex::new(false)),
        console_session: Arc::new(TokioMutex::new(None)),
        console_terminal: Arc::new(Mutex::new(None)),
        console_ports: Arc::new(Mutex::new(Vec::new())),
        active_connect: Arc::new(Mutex::new(None)),
    });

    let is_first_run = !state.vault.lock().unwrap().is_initialized().unwrap_or(false);
    ui.global::<VaultModel>().set_is_first_run(is_first_run);

    // Font & tema terminal — SEKALI di sini (bukan tiap kali panel
    // pengaturan dibuka), daftarnya statis selama app jalan.
    let tm = ui.global::<TerminalTabsModel>();
    tm.set_available_fonts(ModelRc::new(VecModel::from(scan_monospace_fonts())));
    let built_in_themes = terminus_term_emulator::palette::built_in_themes();
    tm.set_available_themes(ModelRc::new(VecModel::from(
        built_in_themes.iter().map(|(name, _)| slint::SharedString::from(*name)).collect::<Vec<_>>(),
    )));
    // Swatch preview per tema (index SEJAJAR sama `available-themes` di
    // atas) — dipakai `ThemeOption` (`terminal-settings.slint`) buat
    // mini-preview kotak warna, langsung dari `Palette` ASLI tiap tema
    // (background + ansi[2] hijau, representatif "baris kode"), bukan
    // warna hiasan.
    tm.set_theme_swatch_bg(ModelRc::new(VecModel::from(
        built_in_themes
            .iter()
            .map(|(_, p)| slint::Color::from_rgb_u8(p.background.r, p.background.g, p.background.b))
            .collect::<Vec<_>>(),
    )));
    tm.set_theme_swatch_accent(ModelRc::new(VecModel::from(
        built_in_themes
            .iter()
            .map(|(_, p)| slint::Color::from_rgb_u8(p.ansi[2].r, p.ansi[2].g, p.ansi[2].b))
            .collect::<Vec<_>>(),
    )));
    // Background terminal ikut tema AKTIF (`state.terminal_theme`,
    // default `terminus_dark()`) — dulu TIDAK ada, `TerminalView`/
    // wrapper-nya pakai token statis yang tidak ikut ganti tema, lihat
    // komentar panjang `terminal-bg-color` di models.slint.
    {
        let bg = state.terminal_theme.lock().unwrap_or_else(|e| e.into_inner()).background;
        tm.set_terminal_bg_color(slint::Color::from_rgb_u8(bg.r, bg.g, bg.b));
    }

    wire_vault_callbacks(ui, &state);
    wire_hosts_callbacks(ui, &state);
    wire_terminal_callbacks(ui, &state);
    wire_sftp_callbacks(ui, &state);
    wire_console_callbacks(ui, &state);
    wire_connecting_callbacks(ui, &state);
    state
}

fn wire_vault_callbacks(ui: &AppWindow, state: &Arc<AppState>) {
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.global::<VaultModel>().on_create_requested(move |password: slint::SharedString| {
            let Some(ui) = ui_weak.upgrade() else { return };
            if ui.global::<VaultModel>().get_busy() {
                return; // sudah ada request jalan, abaikan klik dobel
            }
            ui.global::<VaultModel>().set_busy(true);

            let vault = state.vault.clone();
            let password = password.to_string();
            let ui_weak_task = ui_weak.clone();
            let state_task = state.clone();
            tokio::spawn(async move {
                // Argon2id genuinely CPU-bound & lambat by design —
                // `spawn_blocking` biar tidak numpang worker thread
                // async biasa (yang isinya tugas-tugas ringan lain).
                let result = tokio::task::spawn_blocking(move || vault.lock().unwrap().initialize(&password))
                    .await
                    .expect("blocking task panik");

                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = ui_weak_task.upgrade() else { return };
                    ui.global::<VaultModel>().set_busy(false);
                    match result {
                        Ok(()) => {
                            ui.global::<VaultModel>().set_error_message("".into());
                            ui.global::<VaultModel>().set_is_unlocked(true);
                            refresh_hosts_model(&ui, &state_task);
                        }
                        Err(e) => ui.global::<VaultModel>().set_error_message(e.to_string().into()),
                    }
                });
            });
        });
    }
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.global::<VaultModel>().on_unlock_requested(move |password: slint::SharedString| {
            let Some(ui) = ui_weak.upgrade() else { return };
            if ui.global::<VaultModel>().get_busy() {
                return;
            }
            ui.global::<VaultModel>().set_busy(true);

            let vault = state.vault.clone();
            let password = password.to_string();
            let ui_weak_task = ui_weak.clone();
            let state_task = state.clone();
            tokio::spawn(async move {
                let result = tokio::task::spawn_blocking(move || vault.lock().unwrap().unlock(&password))
                    .await
                    .expect("blocking task panik");

                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = ui_weak_task.upgrade() else { return };
                    ui.global::<VaultModel>().set_busy(false);
                    match result {
                        Ok(()) => {
                            ui.global::<VaultModel>().set_error_message("".into());
                            ui.global::<VaultModel>().set_is_unlocked(true);
                            refresh_hosts_model(&ui, &state_task);
                        }
                        Err(e) => ui.global::<VaultModel>().set_error_message(e.to_string().into()),
                    }
                });
            });
        });
    }
}

fn wire_hosts_callbacks(ui: &AppWindow, state: &Arc<AppState>) {
    // --- Shortcut Ctrl+S -> fokus kotak pencarian. Murni UI state
    //     (tidak sentuh vault sama sekali), tapi TETAP harus lewat
    //     Rust — `HostsModel.focus-search-requested` (global) tidak
    //     bisa di-"handle" langsung dari Slint di `app-window.slint`
    //     (bukan sintaks yang valid), jadi diteruskan ke situ lewat
    //     `AppWindow::focus-search()` (public function di root),
    //     satu-satunya cara Rust nyentuh instance `window-bar` yang
    //     nested. Lihat komentar panjang di app-window.slint. ---
    {
        let ui_weak = ui.as_weak();
        ui.global::<HostsModel>().on_focus_search_requested(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            ui.invoke_focus_search();
        });
    }

    // --- New host ---
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.global::<HostsModel>().on_create_host_requested(move |form| {
            let Some(ui) = ui_weak.upgrade() else { return };
            if ui.global::<HostsModel>().get_creating_host() {
                return;
            }
            ui.global::<HostsModel>().set_creating_host(true);

            let credential_id = Uuid::new_v4();
            let kind = if form.kind == "cisco_ios" { ConnectionKind::CiscoIos } else { ConnectionKind::Ssh };
            let group_id = Uuid::parse_str(&form.group_id).ok();
            let port: u16 = form.port.clamp(1, 65535) as u16;
            let profile = HostProfile {
                id: Uuid::new_v4(),
                label: form.label.to_string(),
                host: form.host.to_string(),
                port,
                username: form.username.to_string(),
                kind,
                auth: AuthMethod::Password { credential_id },
                group_id,
                tags: parse_tags(&form.tags),
            };
            let password_bytes = form.password.as_bytes().to_vec();

            let vault = state.vault.clone();
            let ui_weak_task = ui_weak.clone();
            let state_task = state.clone();
            let profile_for_panel = profile.clone();
            tokio::spawn(async move {
                let result = tokio::task::spawn_blocking(move || {
                    let mut vault = vault.lock().unwrap();
                    vault.store_secret(credential_id, &password_bytes)?;
                    vault.save_profile(&profile)
                })
                .await
                .expect("blocking task panik");

                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = ui_weak_task.upgrade() else { return };
                    ui.global::<HostsModel>().set_creating_host(false);
                    match result {
                        Ok(()) => {
                            // Slide-over TIDAK ketutup — "berubah jadi"
                            // mode edit buat host yang baru dibuat ini
                            // (persis alur host-selected), biar user
                            // bisa lanjut Connect tanpa navigasi ulang.
                            populate_panel(&ui, &state_task, &profile_for_panel);
                            refresh_hosts_model(&ui, &state_task);
                        }
                        Err(e) => show_notice(&ui, &format!("Gagal simpan host: {e}"), true),
                    }
                });
            });
        });
    }

    // --- New group ---
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.global::<HostsModel>().on_create_group_requested(move |name: slint::SharedString, subtitle: slint::SharedString| {
            let Some(ui) = ui_weak.upgrade() else { return };
            if ui.global::<HostsModel>().get_creating_group() {
                return;
            }
            ui.global::<HostsModel>().set_creating_group(true);

            let group = HostGroup {
                id: Uuid::new_v4(),
                name: name.to_string(),
                subtitle: if subtitle.is_empty() { None } else { Some(subtitle.to_string()) },
                parent_id: None,
            };

            let vault = state.vault.clone();
            let ui_weak_task = ui_weak.clone();
            let state_task = state.clone();
            tokio::spawn(async move {
                let result = tokio::task::spawn_blocking(move || vault.lock().unwrap().save_group(&group))
                    .await
                    .expect("blocking task panik");

                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = ui_weak_task.upgrade() else { return };
                    ui.global::<HostsModel>().set_creating_group(false);
                    match result {
                        Ok(()) => {
                            ui.global::<HostsModel>().set_show_new_group_dialog(false);
                            refresh_hosts_model(&ui, &state_task);
                        }
                        Err(e) => show_notice(&ui, &format!("Gagal simpan grup: {e}"), true),
                    }
                });
            });
        });
    }

    // --- Drill-down ke satu grup (baca cepat, tetap sinkron — bukan
    //     mutasi, tidak butuh state loading) ---
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.global::<HostsModel>().on_group_opened(move |group_id: slint::SharedString| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let Ok(uuid) = Uuid::parse_str(&group_id) else { return };
            let name = state
                .vault
                .lock()
                .unwrap()
                .list_groups()
                .ok()
                .and_then(|groups| groups.into_iter().find(|g| g.id == uuid).map(|g| g.name));
            // Set eksplisit di sini (bukan cuma mengandalkan sisi Slint
            // yang sudah men-set ini sebelum manggil callback) — biar
            // handler ini "self-sufficient" tidak bergantung urutan
            // assignment di pemanggil.
            ui.global::<HostsModel>().set_selected_group_id(group_id);
            ui.global::<HostsModel>().set_selected_group_name(name.unwrap_or_default().into());
            refresh_hosts_model(&ui, &state);
        });
    }

    // --- Pencarian global (kotak di TopBar). Beda dari List/Groups:
    //     nyari di SEMUA host dari vault langsung (`list_all_profiles`),
    //     bukan cuma yang lagi ke-cache di `ungrouped-hosts`/
    //     `active-group-hosts` — jadi host di dalam GRUP MANAPUN tetap
    //     ketemu, bukan cuma yang lagi di-drill-down. Baca cepat &
    //     sinkron (bukan mutasi), tidak butuh state loading. ---
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.global::<HostsModel>().on_search_requested(move |query: slint::SharedString| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let query = query.to_string().trim().to_lowercase();
            let results: Vec<HostItem> = if query.is_empty() {
                Vec::new()
            } else {
                let vault = state.vault.lock().unwrap();
                vault
                    .list_all_profiles()
                    .unwrap_or_default()
                    .iter()
                    .filter(|p| host_matches_query(p, &query))
                    .map(|p| host_profile_to_item(&state, p))
                    .collect()
            };
            ui.global::<HostsModel>().set_search_results(ModelRc::new(VecModel::from(results)));
        });
    }

    // --- Klik kartu host = select: baca profil lengkap dari vault
    //     (id, host, port, username, kind, group, tags — BUKAN
    //     password, itu sengaja tetap terenkripsi & tidak pernah
    //     ditampilkan) lalu isi `HostsModel.panel-host-*` supaya panel
    //     "Host Details" di kanan (pola Termius) tampil kepenuhan.
    //     Baca cepat & sinkron, tidak butuh state loading. ---
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.global::<HostsModel>().on_host_selected(move |host_id: slint::SharedString| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let Ok(uuid) = Uuid::parse_str(&host_id) else { return };

            let profile = {
                let vault = state.vault.lock().unwrap();
                vault.list_all_profiles().ok().and_then(|list| list.into_iter().find(|p| p.id == uuid))
            };
            let Some(profile) = profile else {
                show_notice(&ui, "Host tidak ditemukan", true);
                return;
            };

            populate_panel(&ui, &state, &profile);
            ui.global::<HostsModel>().set_panel_visible(true);
        });
    }

    // --- Tombol "Simpan" di panel. Password kosong = TIDAK diubah;
    //     `credential_id` dipertahankan dari profil lama, cuma field
    //     lain (termasuk tags) yang di-overwrite. Setelah sukses,
    //     panel di-refresh ulang dari data yang BARU tersimpan. ---
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.global::<HostsModel>().on_host_save_requested(move |host_id: slint::SharedString, form: NewHostForm| {
            let Some(ui) = ui_weak.upgrade() else { return };
            if ui.global::<HostsModel>().get_panel_busy() {
                return;
            }
            let Ok(uuid) = Uuid::parse_str(&host_id) else { return };

            let existing = {
                let vault = state.vault.lock().unwrap();
                vault.list_all_profiles().ok().and_then(|list| list.into_iter().find(|p| p.id == uuid))
            };
            let Some(existing) = existing else {
                show_notice(&ui, "Host tidak ditemukan", true);
                return;
            };
            let credential_id = match existing.auth {
                AuthMethod::Password { credential_id } => credential_id,
                _ => {
                    show_notice(&ui, "Metode autentikasi host ini belum didukung untuk diedit", true);
                    return;
                }
            };

            ui.global::<HostsModel>().set_panel_busy(true);
            let kind = if form.kind == "cisco_ios" { ConnectionKind::CiscoIos } else { ConnectionKind::Ssh };
            let group_id = Uuid::parse_str(&form.group_id).ok();
            let port: u16 = form.port.clamp(1, 65535) as u16;
            let updated = HostProfile {
                id: uuid,
                label: form.label.to_string(),
                host: form.host.to_string(),
                port,
                username: form.username.to_string(),
                kind,
                auth: AuthMethod::Password { credential_id },
                group_id,
                tags: parse_tags(&form.tags),
            };
            let new_password = if form.password.is_empty() { None } else { Some(form.password.as_bytes().to_vec()) };

            let vault = state.vault.clone();
            let ui_weak_task = ui_weak.clone();
            let state_task = state.clone();
            let updated_for_task = updated.clone();
            tokio::spawn(async move {
                let result = tokio::task::spawn_blocking(move || {
                    let mut vault = vault.lock().unwrap();
                    if let Some(pw) = new_password {
                        vault.store_secret(credential_id, &pw)?;
                    }
                    vault.save_profile(&updated_for_task)
                })
                .await
                .expect("blocking task panik");

                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = ui_weak_task.upgrade() else { return };
                    ui.global::<HostsModel>().set_panel_busy(false);
                    match result {
                        Ok(()) => {
                            show_notice(&ui, "Host berhasil diperbarui", false);
                            populate_panel(&ui, &state_task, &updated);
                            refresh_hosts_model(&ui, &state_task);
                        }
                        Err(e) => show_notice(&ui, &format!("Gagal simpan perubahan host: {e}"), true),
                    }
                });
            });
        });
    }

    // --- Tombol "Duplicate" di panel — deep copy: id & credential_id
    //     BARU, secret didecrypt dari yang lama lalu disimpan ulang di
    //     credential_id baru (bukan berbagi referensi — ubah password
    //     salinan tidak boleh mempengaruhi aslinya). Hasil duplikat
    //     langsung jadi host yang tampil di panel. ---
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.global::<HostsModel>().on_host_duplicate_requested(move |host_id: slint::SharedString| {
            let Some(ui) = ui_weak.upgrade() else { return };
            if ui.global::<HostsModel>().get_panel_busy() {
                return;
            }
            let Ok(uuid) = Uuid::parse_str(&host_id) else { return };

            let existing = {
                let vault = state.vault.lock().unwrap();
                vault.list_all_profiles().ok().and_then(|list| list.into_iter().find(|p| p.id == uuid))
            };
            let Some(existing) = existing else {
                show_notice(&ui, "Host tidak ditemukan", true);
                return;
            };
            let old_credential_id = match existing.auth {
                AuthMethod::Password { credential_id } => credential_id,
                _ => {
                    show_notice(&ui, "Metode autentikasi host ini belum didukung untuk diduplikasi", true);
                    return;
                }
            };

            ui.global::<HostsModel>().set_panel_busy(true);
            let vault = state.vault.clone();
            let ui_weak_task = ui_weak.clone();
            let state_task = state.clone();
            tokio::spawn(async move {
                let result = tokio::task::spawn_blocking(move || -> Result<HostProfile, String> {
                    let mut vault = vault.lock().unwrap();
                    let secret = vault.read_secret(old_credential_id).map_err(|e| e.to_string())?;
                    let new_credential_id = Uuid::new_v4();
                    vault.store_secret(new_credential_id, &secret).map_err(|e| e.to_string())?;
                    let duplicate = HostProfile {
                        id: Uuid::new_v4(),
                        label: format!("{} (copy)", existing.label),
                        auth: AuthMethod::Password { credential_id: new_credential_id },
                        ..existing
                    };
                    vault.save_profile(&duplicate).map_err(|e| e.to_string())?;
                    Ok(duplicate)
                })
                .await
                .expect("blocking task panik");

                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = ui_weak_task.upgrade() else { return };
                    ui.global::<HostsModel>().set_panel_busy(false);
                    match result {
                        Ok(duplicate) => {
                            show_notice(&ui, "Host berhasil diduplikasi", false);
                            populate_panel(&ui, &state_task, &duplicate);
                            ui.global::<HostsModel>().set_panel_visible(true);
                            refresh_hosts_model(&ui, &state_task);
                        }
                        Err(e) => show_notice(&ui, &format!("Gagal menduplikasi host: {e}"), true),
                    }
                });
            });
        });
    }

    // --- Hapus host — dipanggil dari dialog konfirmasi (`ui/components
    //     /confirm-dialog.slint`), BUKAN langsung dari tombol "Remove"
    //     di panel lagi (lihat komentar `HostsModel.confirm-delete-*`
    //     di models.slint). Dialog tetap kebuka sampai ini beneran
    //     selesai (`deleting-host` true), baru Rust yang nutupnya. ---
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.global::<HostsModel>().on_host_delete_requested(move |host_id: slint::SharedString| {
            let Some(ui) = ui_weak.upgrade() else { return };
            if ui.global::<HostsModel>().get_deleting_host() {
                return;
            }
            let Ok(uuid) = Uuid::parse_str(&host_id) else { return };
            ui.global::<HostsModel>().set_deleting_host(true);

            let vault = state.vault.clone();
            let ui_weak_task = ui_weak.clone();
            let state_task = state.clone();
            tokio::spawn(async move {
                let result = tokio::task::spawn_blocking(move || vault.lock().unwrap().delete_profile(uuid))
                    .await
                    .expect("blocking task panik");

                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = ui_weak_task.upgrade() else { return };
                    ui.global::<HostsModel>().set_deleting_host(false);
                    match result {
                        Ok(()) => {
                            ui.global::<HostsModel>().set_confirm_delete_open(false);
                            state_task.statuses.lock().unwrap().remove(&uuid);
                            // Kalau host yang dihapus adalah yang lagi
                            // tampil di panel, tutup panelnya — jangan
                            // biarkan nampilin host yang sudah tidak ada.
                            if ui.global::<HostsModel>().get_panel_host_id() == host_id {
                                ui.global::<HostsModel>().set_panel_visible(false);
                                ui.global::<HostsModel>().set_panel_host_id("".into());
                            }
                            refresh_hosts_model(&ui, &state_task);
                        }
                        Err(e) => show_notice(&ui, &format!("Gagal hapus host: {e}"), true),
                    }
                });
            });
        });
    }

    // --- Hapus grup — dipanggil dari dialog konfirmasi
    //     (`ConfirmDeleteGroupDialog`), BUKAN langsung dari tombol "×"
    //     di kartu grup lagi (lihat komentar `HostsModel.confirm-
    //     delete-group-*` di models.slint). `delete_group` di vault
    //     SEKARANG CASCADE: host DI DALAMNYA (beserta password
    //     tersimpannya) ikut kehapus, bukan cuma diungrouped kayak
    //     sebelumnya — perubahan perilaku ini SENGAJA, atas permintaan
    //     eksplisit user. Sama pola loading-state dengan hapus host:
    //     dialog tetap kebuka sampai beneran selesai. ---
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.global::<HostsModel>().on_group_delete_requested(move |group_id: slint::SharedString| {
            let Some(ui) = ui_weak.upgrade() else { return };
            if ui.global::<HostsModel>().get_deleting_group() {
                return;
            }
            let Ok(uuid) = Uuid::parse_str(&group_id) else { return };
            ui.global::<HostsModel>().set_deleting_group(true);

            let vault = state.vault.clone();
            let ui_weak_task = ui_weak.clone();
            let state_task = state.clone();
            tokio::spawn(async move {
                let result = tokio::task::spawn_blocking(move || vault.lock().unwrap().delete_group(uuid))
                    .await
                    .expect("blocking task panik");

                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = ui_weak_task.upgrade() else { return };
                    ui.global::<HostsModel>().set_deleting_group(false);
                    match result {
                        Ok(()) => {
                            ui.global::<HostsModel>().set_confirm_delete_group_open(false);
                            state_task.selected_groups.lock().unwrap().remove(&uuid);
                            if ui.global::<HostsModel>().get_selected_group_id() == group_id {
                                ui.global::<HostsModel>().set_selected_group_id("".into());
                            }
                            refresh_hosts_model(&ui, &state_task);
                        }
                        Err(e) => show_notice(&ui, &format!("Gagal hapus grup: {e}"), true),
                    }
                });
            });
        });
    }

    // --- Bulk delete: toggle checkbox host/grup — Rust pegang
    //     `HashSet<Uuid>`-nya (lihat komentar `AppState.selected_hosts`
    //     /`selected_groups`), Slint cuma tampilan (`checked` field di
    //     `HostItem`/`GroupItem`, diisi ulang lewat `refresh_hosts_
    //     model`) + `selected-*-count` buat label tombol "Hapus
    //     Terpilih (N)" di toolbar. ---
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.global::<HostsModel>().on_host_selection_toggled(move |host_id: slint::SharedString| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let Ok(uuid) = Uuid::parse_str(&host_id) else { return };
            let count = {
                let mut selected = state.selected_hosts.lock().unwrap();
                if !selected.remove(&uuid) {
                    selected.insert(uuid);
                }
                selected.len() as i32
            };
            ui.global::<HostsModel>().set_selected_host_count(count);
            refresh_hosts_model(&ui, &state);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.global::<HostsModel>().on_group_selection_toggled(move |group_id: slint::SharedString| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let Ok(uuid) = Uuid::parse_str(&group_id) else { return };
            let count = {
                let mut selected = state.selected_groups.lock().unwrap();
                if !selected.remove(&uuid) {
                    selected.insert(uuid);
                }
                selected.len() as i32
            };
            ui.global::<HostsModel>().set_selected_group_count(count);
            refresh_hosts_model(&ui, &state);
        });
    }

    // --- "Select All" (header checkbox `HostGrid`/`GroupGrid`) — Slint
    //     kirim APA ADANYA `root.items` (array yang lagi tampil di grid
    //     itu, BUKAN semua host/grup di vault) + `select` (true = tambah
    //     semua ke seleksi, false = keluarkan semua dari seleksi). ---
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.global::<HostsModel>().on_select_all_hosts_requested(move |select: bool, items: ModelRc<HostItem>| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let count = {
                let mut selected = state.selected_hosts.lock().unwrap();
                for item in items.iter() {
                    let Ok(uuid) = Uuid::parse_str(&item.id) else { continue };
                    if select {
                        selected.insert(uuid);
                    } else {
                        selected.remove(&uuid);
                    }
                }
                selected.len() as i32
            };
            ui.global::<HostsModel>().set_selected_host_count(count);
            refresh_hosts_model(&ui, &state);
        });
    }
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.global::<HostsModel>().on_select_all_groups_requested(move |select: bool, items: ModelRc<GroupItem>| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let count = {
                let mut selected = state.selected_groups.lock().unwrap();
                for item in items.iter() {
                    let Ok(uuid) = Uuid::parse_str(&item.id) else { continue };
                    if select {
                        selected.insert(uuid);
                    } else {
                        selected.remove(&uuid);
                    }
                }
                selected.len() as i32
            };
            ui.global::<HostsModel>().set_selected_group_count(count);
            refresh_hosts_model(&ui, &state);
        });
    }

    // --- Bulk delete: eksekusi beneran (dipanggil dari
    //     `ConfirmBulkDeleteDialog`, sama pola loading/tutup-dialog
    //     dengan hapus host/grup satuan di atas). Host & grup pakai
    //     handler TERPISAH (bukan satu handler generic) karena
    //     operasi vault-nya beda (`delete_profile` vs `delete_group`,
    //     yang terakhir CASCADE ke host di dalamnya juga). ---
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.global::<HostsModel>().on_bulk_delete_hosts_requested(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            if ui.global::<HostsModel>().get_deleting_bulk() {
                return;
            }
            let ids: Vec<Uuid> = state.selected_hosts.lock().unwrap().iter().copied().collect();
            if ids.is_empty() {
                ui.global::<HostsModel>().set_confirm_bulk_delete_open(false);
                return;
            }
            ui.global::<HostsModel>().set_deleting_bulk(true);

            let vault = state.vault.clone();
            let ui_weak_task = ui_weak.clone();
            let state_task = state.clone();
            let ids_for_blocking = ids.clone();
            tokio::spawn(async move {
                let result = tokio::task::spawn_blocking(move || {
                    let mut vault = vault.lock().unwrap();
                    for id in &ids_for_blocking {
                        vault.delete_profile(*id)?;
                    }
                    Ok::<_, terminus_vault::VaultError>(())
                })
                .await
                .expect("blocking task panik");

                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = ui_weak_task.upgrade() else { return };
                    ui.global::<HostsModel>().set_deleting_bulk(false);
                    match result {
                        Ok(()) => {
                            let mut selected = state_task.selected_hosts.lock().unwrap();
                            let mut statuses = state_task.statuses.lock().unwrap();
                            for id in &ids {
                                selected.remove(id);
                                statuses.remove(id);
                            }
                            drop(selected);
                            drop(statuses);
                            ui.global::<HostsModel>().set_selected_host_count(0);
                            ui.global::<HostsModel>().set_confirm_bulk_delete_open(false);
                            if ids.iter().any(|id| ui.global::<HostsModel>().get_panel_host_id() == id.to_string()) {
                                ui.global::<HostsModel>().set_panel_visible(false);
                                ui.global::<HostsModel>().set_panel_host_id("".into());
                            }
                            refresh_hosts_model(&ui, &state_task);
                        }
                        Err(e) => show_notice(&ui, &format!("Gagal hapus host terpilih: {e}"), true),
                    }
                });
            });
        });
    }
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.global::<HostsModel>().on_bulk_delete_groups_requested(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            if ui.global::<HostsModel>().get_deleting_bulk() {
                return;
            }
            let ids: Vec<Uuid> = state.selected_groups.lock().unwrap().iter().copied().collect();
            if ids.is_empty() {
                ui.global::<HostsModel>().set_confirm_bulk_delete_open(false);
                return;
            }
            ui.global::<HostsModel>().set_deleting_bulk(true);

            let vault = state.vault.clone();
            let ui_weak_task = ui_weak.clone();
            let state_task = state.clone();
            let ids_for_blocking = ids.clone();
            tokio::spawn(async move {
                let result = tokio::task::spawn_blocking(move || {
                    let mut vault = vault.lock().unwrap();
                    for id in &ids_for_blocking {
                        vault.delete_group(*id)?;
                    }
                    Ok::<_, terminus_vault::VaultError>(())
                })
                .await
                .expect("blocking task panik");

                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = ui_weak_task.upgrade() else { return };
                    ui.global::<HostsModel>().set_deleting_bulk(false);
                    match result {
                        Ok(()) => {
                            let mut selected = state_task.selected_groups.lock().unwrap();
                            for id in &ids {
                                selected.remove(id);
                            }
                            drop(selected);
                            ui.global::<HostsModel>().set_selected_group_count(0);
                            ui.global::<HostsModel>().set_confirm_bulk_delete_open(false);
                            if ids.iter().any(|id| ui.global::<HostsModel>().get_selected_group_id() == id.to_string()) {
                                ui.global::<HostsModel>().set_selected_group_id("".into());
                            }
                            refresh_hosts_model(&ui, &state_task);
                        }
                        Err(e) => show_notice(&ui, &format!("Gagal hapus grup terpilih: {e}"), true),
                    }
                });
            });
        });
    }

    // --- Connect. Sudah ada tab terbuka buat host ini? Jangan buka
    //     koneksi baru, cukup pindah tampilan (lihat `switch_to_tab`).
    //     Kalau belum, connect BENERAN lalu buka tab baru — INI (bukan
    //     halaman "Console", yang artinya beda: koneksi serial/console
    //     ala minicom) tempat sesi SSH interaktif muncul. ---
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.global::<HostsModel>().on_host_connect_requested(move |host_id: slint::SharedString| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let Ok(uuid) = Uuid::parse_str(&host_id) else { return };

            let already_connected = state.sessions.try_lock().map(|s| s.contains_key(&uuid)).unwrap_or(false);
            if already_connected {
                switch_to_tab(&ui, &state, uuid);
                return;
            }

            let profile = {
                let vault = state.vault.lock().unwrap();
                vault.list_all_profiles().ok().and_then(|list| list.into_iter().find(|p| p.id == uuid))
            };
            let Some(profile) = profile else {
                show_notice(&ui, "Host tidak ditemukan", true);
                return;
            };

            let credential_id = match &profile.auth {
                AuthMethod::Password { credential_id } => *credential_id,
                _ => {
                    show_notice(&ui, "Metode autentikasi ini belum didukung", true);
                    return;
                }
            };
            let password = {
                let vault = state.vault.lock().unwrap();
                match vault.read_secret(credential_id) {
                    Ok(bytes) => String::from_utf8_lossy(&bytes).to_string(),
                    Err(e) => {
                        // Skenario paling umum kena ini: host hasil
                        // Import (lihat `on_import_xml_requested`) yang
                        // memang belum punya password tersimpan sama
                        // sekali — kasih hint eksplisit, bukan cuma
                        // pesan error database mentah.
                        show_notice(
                            &ui,
                            &format!("Gagal baca kredensial (mungkin belum diisi password): {e}"),
                            true,
                        );
                        return;
                    }
                }
            };

            let ui_weak_task = ui_weak.clone();
            let state_task = state.clone();
            track_connect_task(&state, Some(uuid), perform_connect(ui_weak_task, state_task, profile, password));
        });
    }

    // --- Connect langsung dari slide-over mode "New Host" — TIDAK ada
    //     data tersimpan buat dipakai (host-nya belum ada), jadi ini
    //     simpan dulu (persis `create-host-requested`) baru langsung
    //     connect pakai profil yang baru dibuat, satu klik. ---
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.global::<HostsModel>().on_host_connect_new_requested(move |form: NewHostForm| {
            let Some(ui) = ui_weak.upgrade() else { return };
            if ui.global::<HostsModel>().get_panel_busy() {
                return;
            }
            ui.global::<HostsModel>().set_panel_busy(true);

            let credential_id = Uuid::new_v4();
            let kind = if form.kind == "cisco_ios" { ConnectionKind::CiscoIos } else { ConnectionKind::Ssh };
            let group_id = Uuid::parse_str(&form.group_id).ok();
            let port: u16 = form.port.clamp(1, 65535) as u16;
            let profile = HostProfile {
                id: Uuid::new_v4(),
                label: form.label.to_string(),
                host: form.host.to_string(),
                port,
                username: form.username.to_string(),
                kind,
                auth: AuthMethod::Password { credential_id },
                group_id,
                tags: parse_tags(&form.tags),
            };
            let password = form.password.to_string();
            let password_bytes = password.clone().into_bytes();

            let new_host_id = profile.id;
            let vault = state.vault.clone();
            let ui_weak_task = ui_weak.clone();
            let state_task = state.clone();
            let profile_for_connect = profile.clone();
            let profile_for_panel = profile.clone();
            let fut = async move {
                let save_result = tokio::task::spawn_blocking(move || {
                    let mut vault = vault.lock().unwrap();
                    vault.store_secret(credential_id, &password_bytes)?;
                    vault.save_profile(&profile)
                })
                .await
                .expect("blocking task panik");

                let save_succeeded = save_result.is_ok();
                let ui_weak_ui = ui_weak_task.clone();
                let state_ui = state_task.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = ui_weak_ui.upgrade() else { return };
                    ui.global::<HostsModel>().set_panel_busy(false);
                    if let Err(e) = save_result {
                        show_notice(&ui, &format!("Gagal simpan host: {e}"), true);
                        return;
                    }
                    // Slide-over "berubah jadi" mode edit host yang
                    // baru dibuat ini, persis alur `create-host-
                    // requested` biasa — Connect-nya sendiri jalan di
                    // background (di bawah), tidak perlu nunggu ini.
                    populate_panel(&ui, &state_ui, &profile_for_panel);
                    refresh_hosts_model(&ui, &state_ui);
                });

                if save_succeeded {
                    perform_connect(ui_weak_task, state_task, profile_for_connect, password).await;
                }
            };
            track_connect_task(&state, Some(new_host_id), fut);
        });
    }

    // --- Import dari config.xml (SecureCRT) — lihat
    //     `terminus_core::import::securecrt` buat detail parsing-nya.
    //     Dialog "Open File" native (`rfd`) beneran BLOCKING (spin
    //     nested event loop-nya sendiri) — sengaja dijalankan di dalam
    //     `spawn_blocking` bareng kerjaan vault-nya, bukan langsung di
    //     UI thread, biar window utama tidak freeze selama dialog
    //     kebuka DAN biar `importing=true` sempat ke-render (lihat
    //     komentar pola async di atas modul ini). ---
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.global::<HostsModel>().on_import_xml_requested(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            if ui.global::<HostsModel>().get_importing() {
                return;
            }
            ui.global::<HostsModel>().set_importing(true);

            let vault = state.vault.clone();
            let ui_weak_task = ui_weak.clone();
            let state_task = state.clone();
            tokio::spawn(async move {
                let outcome = tokio::task::spawn_blocking(move || -> Result<Option<ImportSummary>, String> {
                    let Some(path) = rfd::FileDialog::new()
                        .set_title("Import host dari config.xml (SecureCRT)")
                        .add_filter("XML", &["xml"])
                        .pick_file()
                    else {
                        return Ok(None); // user batal — bukan error
                    };
                    let xml = std::fs::read_to_string(&path).map_err(|e| format!("Gagal baca file: {e}"))?;
                    let parsed = terminus_core::import::securecrt::parse(&xml).map_err(|e| e.to_string())?;
                    let mut vault = vault.lock().unwrap();
                    import_parsed_hosts_into_vault(&mut vault, parsed).map(Some)
                })
                .await
                .expect("blocking task panik");

                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = ui_weak_task.upgrade() else { return };
                    ui.global::<HostsModel>().set_importing(false);
                    match outcome {
                        Ok(None) => {} // user batal pilih file, tidak perlu notice
                        Ok(Some(summary)) => {
                            let msg = if summary.skipped > 0 {
                                format!(
                                    "Import selesai: {} host ditambahkan, {} dilewati (bukan SSH2). Isi password tiap host lewat panel sebelum Connect.",
                                    summary.imported, summary.skipped
                                )
                            } else {
                                format!(
                                    "Import selesai: {} host ditambahkan. Isi password tiap host lewat panel sebelum Connect.",
                                    summary.imported
                                )
                            };
                            show_notice(&ui, &msg, false);
                            refresh_hosts_model(&ui, &state_task);
                        }
                        Err(e) => show_notice(&ui, &format!("Import gagal: {e}"), true),
                    }
                });
            });
        });
    }
}

struct ImportSummary {
    imported: usize,
    skipped: usize,
}

/// Tulis hasil parse (`terminus_core::import::securecrt::parse`) ke
/// vault: satu `HostProfile` per host, satu `HostGroup` per path
/// folder UNIK (di-flatten jadi satu nama, mis. `["ROUTER","BAROKAH"]`
/// -> `"ROUTER / BAROKAH"` — model grup kita sengaja tidak
/// bertingkat). Grup yang namanya SUDAH ADA di vault dipakai ulang,
/// TIDAK dibuat duplikat — penting buat import ulang dari file
/// `config.xml` yang sudah diperbarui.
///
/// Dipisah dari closure `on_import_xml_requested` (bukan cuma inline
/// di situ) supaya bisa dites langsung tanpa perlu buka dialog file
/// beneran — lihat `tests::import_flatten_group_path_dan_dedup_grup`.
fn import_parsed_hosts_into_vault(
    vault: &mut VaultStore,
    parsed: terminus_core::import::securecrt::ParsedImport,
) -> Result<ImportSummary, String> {
    let mut group_cache: HashMap<String, Uuid> =
        vault.list_groups().map_err(|e| e.to_string())?.into_iter().map(|g| (g.name, g.id)).collect();

    let mut imported = 0usize;
    for host in parsed.hosts {
        let group_id = if host.group_path.is_empty() {
            None
        } else {
            let name = host.group_path.join(" / ");
            if let Some(id) = group_cache.get(&name) {
                Some(*id)
            } else {
                let id = Uuid::new_v4();
                let group = HostGroup { id, name: name.clone(), subtitle: None, parent_id: None };
                vault.save_group(&group).map_err(|e| e.to_string())?;
                group_cache.insert(name, id);
                Some(id)
            }
        };

        // TIDAK panggil `store_secret` — password SecureCRT dienkripsi
        // proprietary, tidak bisa diimpor (lihat doc comment modul
        // securecrt). `credential_id` sengaja "menggantung" tanpa
        // secret sampai user isi manual lewat panel.
        let profile = HostProfile {
            id: Uuid::new_v4(),
            label: host.label,
            host: host.host,
            port: host.port,
            username: host.username,
            kind: ConnectionKind::Ssh,
            auth: AuthMethod::Password { credential_id: Uuid::new_v4() },
            group_id,
            tags: vec!["imported".to_string()],
        };
        vault.save_profile(&profile).map_err(|e| e.to_string())?;
        imported += 1;
    }
    Ok(ImportSummary { imported, skipped: parsed.skipped })
}

/// Callback seputar tab-tab terminal SSH (`TerminalTabsModel`, lihat
/// `ui/models.slint`): input keyboard diteruskan ke PTY remote,
/// klik/tutup tab.
fn wire_terminal_callbacks(ui: &AppWindow, state: &Arc<AppState>) {
    // --- Kontrol jendela CUSTOM (`no-frame: true` di app-window.slint
    //     — title bar asli OS dihapus, atas permintaan eksplisit
    //     user) — minimize/maximize/close/drag lewat `slint::Window`
    //     langsung, BUKAN lewat vault/tokio (murni sinkron, tidak ada
    //     I/O), lihat komentar panjang `window-*-requested` di
    //     models.slint. ---
    {
        let ui_weak = ui.as_weak();
        ui.global::<TerminalTabsModel>().on_window_minimize_requested(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            ui.window().set_minimized(true);
        });
    }
    {
        let ui_weak = ui.as_weak();
        ui.global::<TerminalTabsModel>().on_window_maximize_toggle_requested(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let w = ui.window();
            let now_maximized = !w.is_maximized();
            w.set_maximized(now_maximized);
            ui.global::<TerminalTabsModel>().set_window_maximized(now_maximized);
        });
    }
    {
        ui.global::<TerminalTabsModel>().on_window_close_requested(move || {
            // Aplikasi satu-jendela — nutup jendela ARTINYA keluar
            // aplikasi. `quit_event_loop()` (bukan `std::process::exit`)
            // biar `main()` beneran return & destructor (koneksi SSH,
            // dst) jalan normal, bukan hard-kill.
            let _ = slint::quit_event_loop();
        });
    }
    {
        let ui_weak = ui.as_weak();
        ui.global::<TerminalTabsModel>().on_window_drag_requested(move |dx, dy| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let w = ui.window();
            // `dx`/`dy` datang dalam koordinat LOGICAL (dari
            // `TouchArea.mouse-x/y`, lihat komentar di terminal-tabs-
            // bar.slint) — `position()`/`set_position()` sendiri kerja
            // di PHYSICAL, jadi dikonversi lewat `scale_factor()`.
            // TIDAK didukung semua windowing system (lihat catatan
            // Wayland panjang di models.slint) — kalau tidak didukung,
            // `set_position` di sini SENGAJA dibiarkan no-op diam-diam
            // (perilaku Slint sendiri), bukan sesuatu yang bisa
            // di-workaround dari sisi app.
            let scale = w.scale_factor();
            let cur = w.position();
            w.set_position(slint::PhysicalPosition::new(
                cur.x + (dx * scale) as i32,
                cur.y + (dy * scale) as i32,
            ));
        });
    }

    // --- Keyboard -> stdin sesi AKTIF. Tidak ada local echo (lihat
    //     komentar di `TerminalView`), jadi ini murni "kirim byte ini"
    //     — tidak perlu balik ke UI thread sama sekali. ---
    {
        let state = state.clone();
        ui.global::<TerminalTabsModel>().on_pty_input_requested(move |text: slint::SharedString| {
            let Some(host_id) = *state.active_terminal.lock().unwrap_or_else(|e| e.into_inner()) else { return };
            let bytes = text.as_bytes().to_vec();
            let state = state.clone();
            tokio::spawn(async move {
                if let Some(session) = state.sessions.lock().await.get(&host_id) {
                    let _ = session.write(&bytes).await;
                }
            });
        });
    }

    // --- Klik badan tab -> pindah tampilan (sesi lain TETAP hidup). ---
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.global::<TerminalTabsModel>().on_tab_selected(move |tab_id: slint::SharedString| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let Ok(uuid) = Uuid::parse_str(&tab_id) else { return };
            switch_to_tab(&ui, &state, uuid);
        });
    }

    // --- Klik "×" di tab -> putus sesi beneran. UI di-update LANGSUNG
    //     (tidak nunggu round-trip network tutup koneksi) supaya
    //     responsif — penutupan socket beneran jalan di background. ---
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.global::<TerminalTabsModel>().on_tab_close_requested(move |tab_id: slint::SharedString| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let Ok(uuid) = Uuid::parse_str(&tab_id) else { return };

            finalize_tab_closed(&ui, &state, uuid);

            let state_task = state.clone();
            tokio::spawn(async move {
                if let Some(mut session) = state_task.sessions.lock().await.remove(&uuid) {
                    let _ = session.disconnect().await;
                }
            });
        });
    }

    // --- Ganti color theme (panel pengaturan, ⚙ di halaman Terminal).
    //     Sinkron & cepat (murni pilih dari daftar bawaan, tidak ada
    //     I/O), TAPI beda dari font: tema mempengaruhi RESOLUSI WARNA
    //     yang sudah dilakukan `terminus-term-emulator`, jadi konten
    //     yang SUDAH tampil perlu di-render ULANG (lihat
    //     `render_and_cache_tab`) — bukan cuma berlaku ke output
    //     berikutnya. ---
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.global::<TerminalTabsModel>().on_terminal_theme_changed(move |name: slint::SharedString| {
            let Some(theme) = terminus_term_emulator::palette::by_name(&name) else { return };
            // SENGAJA di-`tokio::spawn` (BUKAN dikerjakan langsung
            // inline di callback ini kayak sebelumnya) — laporan user
            // "ganti color theme jadi crash". Root cause paling
            // mungkin: `render_and_cache_tab` (lewat `invoke_from_
            // event_loop`) dipanggil SINKRON dari DALAM callback Slint
            // yang lagi jalan gara-gara property `terminal-theme-name`
            // BARU SAJA diset di sisi Slint SEBELUM callback ini
            // ditembak — memicu `set_rows()` (mutasi model + relayout
            // ~32 baris terminal) REENTRANT ke dalam siklus evaluasi
            // binding Slint yang belum selesai. `tokio::spawn` majukan
            // kerjanya ke iterasi event loop BERIKUTNYA (thread pool
            // tokio, baru `invoke_from_event_loop` genuinely lintas-
            // thread), persis pola yang SUDAH dipakai SEMUA handler
            // lain di file ini (lihat komentar modul di atas) — cuma
            // handler tema ini yang tadinya menyimpang jadi sinkron.
            let ui_weak = ui_weak.clone();
            let state = state.clone();
            tokio::spawn(async move {
                *state.terminal_theme.lock().unwrap_or_else(|e| e.into_inner()) = theme;
                let active_id = *state.active_terminal.lock().unwrap_or_else(|e| e.into_inner());
                // Update background PANEL terminal juga (bukan cuma
                // per-sel teks) — TERLEPAS ada tab aktif atau tidak,
                // biar begitu user buka/pindah tab ke Terminal
                // nanti sudah kepakai. Lihat komentar `terminal-bg-
                // color` di models.slint.
                let bg = slint::Color::from_rgb_u8(theme.background.r, theme.background.g, theme.background.b);
                let ui_weak_bg = ui_weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_weak_bg.upgrade() {
                        ui.global::<TerminalTabsModel>().set_terminal_bg_color(bg);
                    }
                });
                if let Some(active_id) = active_id {
                    render_and_cache_tab(&ui_weak, &state, active_id);
                }
            });
        });
    }
}

/// Semua callback halaman SFTP — connect (saved host lewat vault ATAU
/// manual ad-hoc), disconnect, refresh, navigasi dua panel. Local
/// listing pakai `std::fs` langsung (tidak butuh SFTP), remote lewat
/// `terminus-sftp-engine` beneran (bukan mock lagi).
fn wire_sftp_callbacks(ui: &AppWindow, state: &Arc<AppState>) {
    // --- Connect ke host TERSIMPAN (vault) — sama persis pola baca
    //     kredensial dengan `on_host_connect_requested` (SSH biasa),
    //     bedanya di sini buka `SftpBrowser`, bukan `SshSession`. ---
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.global::<SftpModel>().on_sftp_connect_requested(move |host_id: slint::SharedString| {
            let Some(ui) = ui_weak.upgrade() else { return };
            if ui.global::<SftpModel>().get_connecting() {
                return;
            }
            let Ok(uuid) = Uuid::parse_str(&host_id) else { return };

            let profile = {
                let vault = state.vault.lock().unwrap();
                vault.list_all_profiles().ok().and_then(|list| list.into_iter().find(|p| p.id == uuid))
            };
            let Some(profile) = profile else {
                show_notice(&ui, "Host tidak ditemukan", true);
                return;
            };
            let credential_id = match &profile.auth {
                AuthMethod::Password { credential_id } => *credential_id,
                _ => {
                    show_notice(&ui, "Metode autentikasi ini belum didukung", true);
                    return;
                }
            };
            let password = {
                let vault = state.vault.lock().unwrap();
                match vault.read_secret(credential_id) {
                    Ok(bytes) => String::from_utf8_lossy(&bytes).to_string(),
                    Err(e) => {
                        show_notice(
                            &ui,
                            &format!("Gagal baca kredensial (mungkin belum diisi password): {e}"),
                            true,
                        );
                        return;
                    }
                }
            };

            ui.global::<SftpModel>().set_connecting(true);
            ui.global::<SftpModel>().set_connect_error("".into());

            let ui_weak_task = ui_weak.clone();
            let state_task = state.clone();
            track_connect_task(&state, None, async move {
                sftp_connect_and_refresh(ui_weak_task, state_task, profile, SecretMaterial::Password(password)).await;
            });
        });
    }

    // --- Connect ke host MANUAL (ad-hoc, tidak pernah disimpan ke
    //     vault) — profile "hangat" dibikin di tempat, id acak,
    //     tidak pernah `save_profile()`. ---
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.global::<SftpModel>().on_sftp_connect_manual_requested(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            if ui.global::<SftpModel>().get_connecting() {
                return;
            }
            let sftp_model = ui.global::<SftpModel>();
            let host = sftp_model.get_manual_host().to_string();
            let port: u16 = sftp_model.get_manual_port().parse().unwrap_or(22);
            let username = sftp_model.get_manual_username().to_string();
            let password = sftp_model.get_manual_password().to_string();
            if host.is_empty() || username.is_empty() {
                sftp_model.set_connect_error("Host & username wajib diisi.".into());
                return;
            }

            let profile = HostProfile {
                id: Uuid::new_v4(),
                label: host.clone(),
                host,
                port,
                username,
                kind: ConnectionKind::Ssh,
                // `credential_id` di sini TIDAK PERNAH dipakai buat baca
                // vault (mode manual tidak nyentuh vault sama sekali) —
                // cuma diisi biar `HostProfile` valid, id acak murni.
                auth: AuthMethod::Password { credential_id: Uuid::new_v4() },
                group_id: None,
                tags: Vec::new(),
            };

            sftp_model.set_connecting(true);
            sftp_model.set_connect_error("".into());

            let ui_weak_task = ui_weak.clone();
            let state_task = state.clone();
            track_connect_task(&state, None, async move {
                sftp_connect_and_refresh(ui_weak_task, state_task, profile, SecretMaterial::Password(password)).await;
            });
        });
    }

    // --- Disconnect -> balik ke chooser. Drop `SftpBrowser` (nutup
    //     koneksi SSH-nya) sebelum reset UI state. ---
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.global::<SftpModel>().on_sftp_disconnect_requested(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let state = state.clone();
            tokio::spawn(async move {
                *state.sftp.lock().await = None;
            });
            let sftp_model = ui.global::<SftpModel>();
            sftp_model.set_connected(false);
            sftp_model.set_local_entries(ModelRc::new(VecModel::from(Vec::<FileEntry>::new())));
            sftp_model.set_remote_entries(ModelRc::new(VecModel::from(Vec::<FileEntry>::new())));
        });
    }

    // --- Refresh (Ctrl+R, lihat `page-sftp.slint`) — list ulang DUA
    //     panel di path yang lagi ditampilkan, tanpa reconnect. ---
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.global::<SftpModel>().on_sftp_refresh_requested(move || {
            if ui_weak.upgrade().is_none() {
                return;
            }
            let ui_weak_task = ui_weak.clone();
            let state_task = state.clone();
            tokio::spawn(async move {
                refresh_sftp_panes(ui_weak_task, state_task).await;
            });
        });
    }

    // --- Navigasi panel Local — murni `std::fs`, tidak nyentuh SFTP
    //     sama sekali. ---
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.global::<SftpModel>().on_sftp_local_navigate_requested(move |name: slint::SharedString| {
            if ui_weak.upgrade().is_none() {
                return;
            }
            {
                let mut path = state.sftp_local_path.lock().unwrap_or_else(|e| e.into_inner());
                if name.as_str() == ".." {
                    path.pop();
                } else {
                    path.push(name.as_str());
                }
            }
            // Centangan (Select All/klik satu-satu) TIDAK boleh
            // "ikut kebawa" pindah folder — folder baru mulai bersih.
            state.sftp_local_selected.lock().unwrap_or_else(|e| e.into_inner()).clear();
            let ui_weak_task = ui_weak.clone();
            let state_task = state.clone();
            tokio::spawn(async move {
                refresh_sftp_panes(ui_weak_task, state_task).await;
            });
        });
    }

    // --- Navigasi panel Remote — beneran manggil `list_dir()` lagi ke
    //     server lewat `SftpBrowser` yang lagi aktif. ---
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.global::<SftpModel>().on_sftp_remote_navigate_requested(move |name: slint::SharedString| {
            if ui_weak.upgrade().is_none() {
                return;
            }
            {
                let mut path = state.sftp_remote_path.lock().unwrap_or_else(|e| e.into_inner());
                *path = if name.as_str() == ".." { posix_parent(&path) } else { posix_join(&path, name.as_str()) };
            }
            state.sftp_remote_selected.lock().unwrap_or_else(|e| e.into_inner()).clear();
            let ui_weak_task = ui_weak.clone();
            let state_task = state.clone();
            tokio::spawn(async move {
                refresh_sftp_panes(ui_weak_task, state_task).await;
            });
        });
    }

    // --- Ketik LANGSUNG di path bar (lihat `FilePane.path-submitted`
    //     di page-sftp.slint) lalu Enter -> lompat ke path ABSOLUT itu
    //     (beda dari navigate-requested di atas, yang cuma relatif
    //     satu level lewat klik dua kali folder) — atas permintaan
    //     eksplisit user, path harus bisa diketik/diganti manual. ---
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.global::<SftpModel>().on_sftp_local_path_set_requested(move |path: slint::SharedString| {
            if ui_weak.upgrade().is_none() {
                return;
            }
            *state.sftp_local_path.lock().unwrap_or_else(|e| e.into_inner()) = std::path::PathBuf::from(path.as_str());
            state.sftp_local_selected.lock().unwrap_or_else(|e| e.into_inner()).clear();
            let ui_weak_task = ui_weak.clone();
            let state_task = state.clone();
            tokio::spawn(async move {
                refresh_sftp_panes(ui_weak_task, state_task).await;
            });
        });
    }
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.global::<SftpModel>().on_sftp_remote_path_set_requested(move |path: slint::SharedString| {
            if ui_weak.upgrade().is_none() {
                return;
            }
            // Path SFTP SELALU POSIX ("/"-prefixed) — kalau user ketik
            // tanpa "/" di depan (mis. "var/www"), tambahin sendiri
            // biar tetap valid sebagai path absolut, bukan diperlakukan
            // sebagai relatif ke path SEBELUMNYA (yang tidak ada
            // artinya buat "ketik langsung ganti path").
            let mut p = path.to_string();
            if !p.starts_with('/') {
                p = format!("/{p}");
            }
            *state.sftp_remote_path.lock().unwrap_or_else(|e| e.into_inner()) = p;
            state.sftp_remote_selected.lock().unwrap_or_else(|e| e.into_inner()).clear();
            let ui_weak_task = ui_weak.clone();
            let state_task = state.clone();
            tokio::spawn(async move {
                refresh_sftp_panes(ui_weak_task, state_task).await;
            });
        });
    }

    // --- Copy Local -> Remote ("upload") — GANTI drag & drop (lihat
    //     komentar panjang di `SftpModel.local-selected-name`,
    //     models.slint, soal kenapa itu tidak bisa dibangun sekarang).
    //     File tunggal saja (bukan folder, dijamin sisi Slint — klik
    //     select cuma nyala buat `!f.is-dir`). ---
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.global::<SftpModel>().on_sftp_copy_to_remote_requested(move |name: slint::SharedString| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let sftp_model = ui.global::<SftpModel>();
            if sftp_model.get_transferring() {
                return;
            }
            sftp_model.set_transferring(true);
            sftp_model.set_transfer_error("".into());

            let local_path = state.sftp_local_path.lock().unwrap_or_else(|e| e.into_inner()).join(name.as_str());
            let remote_path = posix_join(&state.sftp_remote_path.lock().unwrap_or_else(|e| e.into_inner()), name.as_str());

            let ui_weak_task = ui_weak.clone();
            let state_task = state.clone();
            tokio::spawn(async move {
                let bytes = tokio::task::spawn_blocking(move || std::fs::read(&local_path))
                    .await
                    .expect("blocking task panik");

                let result: Result<(), String> = match bytes {
                    Ok(bytes) => {
                        let sftp = state_task.sftp.lock().await;
                        match sftp.as_ref() {
                            Some(browser) => browser.upload(&remote_path, &bytes).await.map_err(|e| e.to_string()),
                            None => Err("sesi SFTP sudah terputus".to_string()),
                        }
                    }
                    Err(e) => Err(format!("gagal baca file lokal: {e}")),
                };

                let ok = result.is_ok();
                let _ = slint::invoke_from_event_loop({
                    let ui_weak_task = ui_weak_task.clone();
                    move || {
                        let Some(ui) = ui_weak_task.upgrade() else { return };
                        let sftp_model = ui.global::<SftpModel>();
                        sftp_model.set_transferring(false);
                        if let Err(e) = result {
                            sftp_model.set_transfer_error(format!("Gagal upload: {e}").into());
                        }
                    }
                });
                if ok {
                    refresh_sftp_panes(ui_weak_task, state_task).await;
                }
            });
        });
    }

    // --- Copy Remote -> Local ("download") — sama pola, arah
    //     kebalikan. ---
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.global::<SftpModel>().on_sftp_copy_to_local_requested(move |name: slint::SharedString| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let sftp_model = ui.global::<SftpModel>();
            if sftp_model.get_transferring() {
                return;
            }
            sftp_model.set_transferring(true);
            sftp_model.set_transfer_error("".into());

            let remote_path = posix_join(&state.sftp_remote_path.lock().unwrap_or_else(|e| e.into_inner()), name.as_str());
            let local_path = state.sftp_local_path.lock().unwrap_or_else(|e| e.into_inner()).join(name.as_str());

            let ui_weak_task = ui_weak.clone();
            let state_task = state.clone();
            tokio::spawn(async move {
                let downloaded: Result<Vec<u8>, String> = {
                    let sftp = state_task.sftp.lock().await;
                    match sftp.as_ref() {
                        Some(browser) => browser.download(&remote_path).await.map_err(|e| e.to_string()),
                        None => Err("sesi SFTP sudah terputus".to_string()),
                    }
                };

                let result: Result<(), String> = match downloaded {
                    Ok(bytes) => tokio::task::spawn_blocking(move || std::fs::write(&local_path, &bytes))
                        .await
                        .expect("blocking task panik")
                        .map_err(|e| format!("gagal tulis file lokal: {e}")),
                    Err(e) => Err(e),
                };

                let ok = result.is_ok();
                let _ = slint::invoke_from_event_loop({
                    let ui_weak_task = ui_weak_task.clone();
                    move || {
                        let Some(ui) = ui_weak_task.upgrade() else { return };
                        let sftp_model = ui.global::<SftpModel>();
                        sftp_model.set_transferring(false);
                        if let Err(e) = result {
                            sftp_model.set_transfer_error(format!("Gagal download: {e}").into());
                        }
                    }
                });
                if ok {
                    refresh_sftp_panes(ui_weak_task, state_task).await;
                }
            });
        });
    }

    // --- Drag & drop beneran antar panel — KOREKSI dari klaim
    //     sebelumnya di sesi ini yang salah bilang ini mustahil pakai
    //     API publik Slint. `slint::DataTransfer` TERNYATA memang
    //     public (cek https://docs.rs/slint/1.17.1/slint/struct.
    //     DataTransfer.html — sebelumnya salah cari re-export-nya di
    //     source lokal). `data-transfer` OPAQUE di sisi Slint, jadi
    //     dua callback `pure` di bawah ini yang jembatani konversinya
    //     ke/dari `string` biasa — payload-nya SEKEDAR "asal:nama"
    //     (mis. "local:foto.png"), dikonstruksi sisi Slint waktu drag
    //     mulai (lihat `FilePane` di page-sftp.slint). ---
    {
        ui.global::<SftpModel>().on_sftp_make_transfer(move |payload: slint::SharedString| {
            slint::DataTransfer::from(payload)
        });
    }
    {
        ui.global::<SftpModel>().on_sftp_read_transfer(move |data: slint::DataTransfer| {
            data.plain_text().unwrap_or_default()
        });
    }
    // --- Drop beneran terjadi — CUKUP panggil ULANG callback "Copy"
    //     yang SUDAH ada (`invoke_sftp_copy_to_*_requested`, PERSIS
    //     sama seolah user klik tombol Copy) — tidak perlu duplikasi
    //     logic transfer sama sekali. `dest` ("panel TEMPAT dijatuhkan)
    //     dibandingkan sama `src` (awalan payload, "panel ASAL drag")
    //     buat nolak drop SEARAH (mis. seret file di dalam panel Local
    //     itu sendiri) — cuma proses kalau dua-duanya BEDA panel. ---
    {
        let ui_weak = ui.as_weak();
        ui.global::<SftpModel>().on_sftp_file_dropped_requested(move |dest: slint::SharedString, payload: slint::SharedString| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let payload = payload.to_string();
            let Some((src, name)) = payload.split_once(':') else { return };
            if src == dest.as_str() {
                return;
            }
            let sftp_model = ui.global::<SftpModel>();
            match dest.as_str() {
                "local" => sftp_model.invoke_sftp_copy_to_local_requested(name.into()),
                "remote" => sftp_model.invoke_sftp_copy_to_remote_requested(name.into()),
                _ => {}
            }
        });
    }

    // --- Menu "select option" ala Termius (lihat page-sftp.slint):
    //     toggle centang satu baris (folder TIDAK bisa dicentang,
    //     dijamin sisi Slint). ---
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.global::<SftpModel>().on_sftp_toggle_check_requested(move |pane: slint::SharedString, name: slint::SharedString| {
            if ui_weak.upgrade().is_none() {
                return;
            }
            let selected = match pane.as_str() {
                "local" => &state.sftp_local_selected,
                "remote" => &state.sftp_remote_selected,
                _ => return,
            };
            {
                let mut set = selected.lock().unwrap_or_else(|e| e.into_inner());
                if !set.remove(name.as_str()) {
                    set.insert(name.to_string());
                }
            }
            let ui_weak_task = ui_weak.clone();
            let state_task = state.clone();
            tokio::spawn(async move {
                refresh_sftp_panes(ui_weak_task, state_task).await;
            });
        });
    }

    // --- Select All — list ULANG direktori itu (biar dapat daftar
    //     nama FILE nyata sekarang, bukan cache basi), centang semua
    //     (atau kosongkan kalau semua sudah tercentang — toggle). ---
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.global::<SftpModel>().on_sftp_select_all_requested(move |pane: slint::SharedString| {
            if ui_weak.upgrade().is_none() {
                return;
            }
            let pane = pane.to_string();
            let ui_weak_task = ui_weak.clone();
            let state_task = state.clone();
            tokio::spawn(async move {
                let names: Vec<String> = match pane.as_str() {
                    "local" => {
                        let path = state_task.sftp_local_path.lock().unwrap_or_else(|e| e.into_inner()).clone();
                        tokio::task::spawn_blocking(move || list_local_dir(&path))
                            .await
                            .unwrap_or_default()
                            .into_iter()
                            .filter(|(_, is_dir, ..)| !is_dir)
                            .map(|(name, ..)| name)
                            .collect()
                    }
                    "remote" => {
                        let path = state_task.sftp_remote_path.lock().unwrap_or_else(|e| e.into_inner()).clone();
                        let sftp = state_task.sftp.lock().await;
                        match sftp.as_ref() {
                            Some(browser) => browser
                                .list_dir(&path)
                                .await
                                .unwrap_or_default()
                                .into_iter()
                                .filter(|e| !e.is_dir)
                                .map(|e| e.name)
                                .collect(),
                            None => Vec::new(),
                        }
                    }
                    _ => return,
                };
                {
                    let selected = match pane.as_str() {
                        "local" => &state_task.sftp_local_selected,
                        "remote" => &state_task.sftp_remote_selected,
                        _ => return,
                    };
                    let mut set = selected.lock().unwrap_or_else(|e| e.into_inner());
                    let all_already_selected = !names.is_empty() && names.iter().all(|n| set.contains(n));
                    set.clear();
                    if !all_already_selected {
                        set.extend(names);
                    }
                }
                refresh_sftp_panes(ui_weak_task, state_task).await;
            });
        });
    }

    // --- Range-select (Shift+klik / Shift+Panah, page-sftp.slint) —
    //     GANTI (bukan tambah) seleksi panel jadi semua file di
    //     antara `anchor`/`target`, INKLUSIF. List ULANG direktori itu
    //     (sama teknik Select All) buat dapat urutan KANONIK — Slint
    //     cuma kirim NAMA, bukan index, jadi tidak ada resiko index
    //     "basi" beda urutan dari yang Rust punya. ---
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.global::<SftpModel>().on_sftp_select_range_requested(
            move |pane: slint::SharedString, anchor: slint::SharedString, target: slint::SharedString| {
                if ui_weak.upgrade().is_none() {
                    return;
                }
                let pane = pane.to_string();
                let anchor = anchor.to_string();
                let target = target.to_string();
                let ui_weak_task = ui_weak.clone();
                let state_task = state.clone();
                tokio::spawn(async move {
                    let names: Vec<String> = match pane.as_str() {
                        "local" => {
                            let path = state_task.sftp_local_path.lock().unwrap_or_else(|e| e.into_inner()).clone();
                            tokio::task::spawn_blocking(move || list_local_dir(&path))
                                .await
                                .unwrap_or_default()
                                .into_iter()
                                .filter(|(_, is_dir, ..)| !is_dir)
                                .map(|(name, ..)| name)
                                .collect()
                        }
                        "remote" => {
                            let path = state_task.sftp_remote_path.lock().unwrap_or_else(|e| e.into_inner()).clone();
                            let sftp = state_task.sftp.lock().await;
                            match sftp.as_ref() {
                                Some(browser) => browser
                                    .list_dir(&path)
                                    .await
                                    .unwrap_or_default()
                                    .into_iter()
                                    .filter(|e| !e.is_dir)
                                    .map(|e| e.name)
                                    .collect(),
                                None => Vec::new(),
                            }
                        }
                        _ => return,
                    };
                    let Some(a_idx) = names.iter().position(|n| n == &anchor) else { return };
                    let Some(t_idx) = names.iter().position(|n| n == &target) else { return };
                    let (lo, hi) = (a_idx.min(t_idx), a_idx.max(t_idx));
                    let range: std::collections::HashSet<String> = names[lo..=hi].iter().cloned().collect();
                    let selected = match pane.as_str() {
                        "local" => &state_task.sftp_local_selected,
                        "remote" => &state_task.sftp_remote_selected,
                        _ => return,
                    };
                    *selected.lock().unwrap_or_else(|e| e.into_inner()) = range;
                    refresh_sftp_panes(ui_weak_task, state_task).await;
                });
            },
        );
    }

    // --- Klik di area kosong panel (bukan di baris file mana pun) ->
    //     hapus seleksi panel itu. ---
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.global::<SftpModel>().on_sftp_clear_selection_requested(move |pane: slint::SharedString| {
            if ui_weak.upgrade().is_none() {
                return;
            }
            let selected = match pane.as_str() {
                "local" => &state.sftp_local_selected,
                "remote" => &state.sftp_remote_selected,
                _ => return,
            };
            selected.lock().unwrap_or_else(|e| e.into_inner()).clear();
            let ui_weak_task = ui_weak.clone();
            let state_task = state.clone();
            tokio::spawn(async move {
                refresh_sftp_panes(ui_weak_task, state_task).await;
            });
        });
    }

    // --- Delete — beroperasi ke SEMUA nama yang lagi dicentang di
    //     panel itu (1 file lewat klik satu-satu, atau banyak lewat
    //     Select All — sama logic-nya). ---
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.global::<SftpModel>().on_sftp_delete_selected_requested(move |pane: slint::SharedString| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let sftp_model = ui.global::<SftpModel>();
            if sftp_model.get_transferring() {
                return;
            }
            let pane = pane.to_string();
            let names: Vec<String> = {
                let selected = match pane.as_str() {
                    "local" => &state.sftp_local_selected,
                    "remote" => &state.sftp_remote_selected,
                    _ => return,
                };
                selected.lock().unwrap_or_else(|e| e.into_inner()).iter().cloned().collect()
            };
            if names.is_empty() {
                return;
            }
            sftp_model.set_transferring(true);
            sftp_model.set_transfer_error("".into());

            let ui_weak_task = ui_weak.clone();
            let state_task = state.clone();
            tokio::spawn(async move {
                let mut last_err: Option<String> = None;
                match pane.as_str() {
                    "local" => {
                        let base = state_task.sftp_local_path.lock().unwrap_or_else(|e| e.into_inner()).clone();
                        for name in &names {
                            let p = base.join(name);
                            if let Err(e) =
                                tokio::task::spawn_blocking(move || std::fs::remove_file(&p)).await.expect("blocking task panik")
                            {
                                last_err = Some(format!("gagal hapus {name}: {e}"));
                            }
                        }
                    }
                    "remote" => {
                        let base = state_task.sftp_remote_path.lock().unwrap_or_else(|e| e.into_inner()).clone();
                        let sftp = state_task.sftp.lock().await;
                        if let Some(browser) = sftp.as_ref() {
                            for name in &names {
                                let p = posix_join(&base, name);
                                if let Err(e) = browser.remove_file(&p).await {
                                    last_err = Some(format!("gagal hapus {name}: {e}"));
                                }
                            }
                        } else {
                            last_err = Some("sesi SFTP sudah terputus".to_string());
                        }
                    }
                    _ => {}
                }
                match pane.as_str() {
                    "local" => state_task.sftp_local_selected.lock().unwrap_or_else(|e| e.into_inner()).clear(),
                    "remote" => state_task.sftp_remote_selected.lock().unwrap_or_else(|e| e.into_inner()).clear(),
                    _ => {}
                }
                let _ = slint::invoke_from_event_loop({
                    let ui_weak_task = ui_weak_task.clone();
                    move || {
                        let Some(ui) = ui_weak_task.upgrade() else { return };
                        let sftp_model = ui.global::<SftpModel>();
                        sftp_model.set_transferring(false);
                        if let Some(e) = last_err {
                            sftp_model.set_transfer_error(e.into());
                        }
                    }
                });
                refresh_sftp_panes(ui_weak_task, state_task).await;
            });
        });
    }

    // --- Delete SATU file — dipakai menu klik-kanan PER BARIS, BUKAN
    //     lewat set centang. Gampang: timpa set centang panel itu
    //     jadi PERSIS 1 nama ini, lalu panggil ULANG handler bulk di
    //     atas (`invoke_sftp_delete_selected_requested`) — reuse total,
    //     tidak duplikasi logic hapus. ---
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.global::<SftpModel>().on_sftp_delete_one_requested(move |pane: slint::SharedString, name: slint::SharedString| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let selected = match pane.as_str() {
                "local" => &state.sftp_local_selected,
                "remote" => &state.sftp_remote_selected,
                _ => return,
            };
            {
                let mut set = selected.lock().unwrap_or_else(|e| e.into_inner());
                set.clear();
                set.insert(name.to_string());
            }
            ui.global::<SftpModel>().invoke_sftp_delete_selected_requested(pane);
        });
    }

    // --- Show Hidden Files — Slint SUDAH flip properti `*-show-hidden`
    //     SEBELUM manggil callback ini (lihat wiring-nya di
    //     page-sftp.slint), jadi tinggal BACA nilai barunya & simpan
    //     ke `AppState` (WAJIB — `refresh_sftp_panes` jalan di
    //     background thread, tidak boleh sentuh Slint). ---
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.global::<SftpModel>().on_sftp_show_hidden_toggled(move |pane: slint::SharedString| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let sftp_model = ui.global::<SftpModel>();
            let (flag, new_value) = match pane.as_str() {
                "local" => (&state.sftp_local_show_hidden, sftp_model.get_local_show_hidden()),
                "remote" => (&state.sftp_remote_show_hidden, sftp_model.get_remote_show_hidden()),
                _ => return,
            };
            *flag.lock().unwrap_or_else(|e| e.into_inner()) = new_value;
            let ui_weak_task = ui_weak.clone();
            let state_task = state.clone();
            tokio::spawn(async move {
                refresh_sftp_panes(ui_weak_task, state_task).await;
            });
        });
    }

    // --- Rename — modal kecil isi `rename-*` (lihat models.slint),
    //     dibaca di sini waktu tombol Confirm-nya diklik. ---
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.global::<SftpModel>().on_sftp_rename_requested(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let sftp_model = ui.global::<SftpModel>();
            if sftp_model.get_transferring() {
                return;
            }
            let pane = sftp_model.get_rename_pane().to_string();
            let old_name = sftp_model.get_rename_old_name().to_string();
            let new_name = sftp_model.get_rename_new_name().to_string();
            if new_name.trim().is_empty() || old_name == new_name {
                sftp_model.set_rename_modal_open(false);
                return;
            }
            sftp_model.set_transferring(true);
            sftp_model.set_transfer_error("".into());

            let ui_weak_task = ui_weak.clone();
            let state_task = state.clone();
            tokio::spawn(async move {
                let result: Result<(), String> = match pane.as_str() {
                    "local" => {
                        let base = state_task.sftp_local_path.lock().unwrap_or_else(|e| e.into_inner()).clone();
                        let old_path = base.join(&old_name);
                        let new_path = base.join(&new_name);
                        tokio::task::spawn_blocking(move || std::fs::rename(&old_path, &new_path))
                            .await
                            .expect("blocking task panik")
                            .map_err(|e| e.to_string())
                    }
                    "remote" => {
                        let base = state_task.sftp_remote_path.lock().unwrap_or_else(|e| e.into_inner()).clone();
                        let old_path = posix_join(&base, &old_name);
                        let new_path = posix_join(&base, &new_name);
                        let sftp = state_task.sftp.lock().await;
                        match sftp.as_ref() {
                            Some(browser) => browser.rename(&old_path, &new_path).await.map_err(|e| e.to_string()),
                            None => Err("sesi SFTP sudah terputus".to_string()),
                        }
                    }
                    _ => Err("panel tidak dikenal".to_string()),
                };
                let ok = result.is_ok();
                let _ = slint::invoke_from_event_loop({
                    let ui_weak_task = ui_weak_task.clone();
                    move || {
                        let Some(ui) = ui_weak_task.upgrade() else { return };
                        let sftp_model = ui.global::<SftpModel>();
                        sftp_model.set_transferring(false);
                        match result {
                            Ok(()) => sftp_model.set_rename_modal_open(false),
                            Err(e) => sftp_model.set_transfer_error(format!("Gagal rename: {e}").into()),
                        }
                    }
                });
                if ok {
                    refresh_sftp_panes(ui_weak_task, state_task).await;
                }
            });
        });
    }

    // --- New Folder — modal kecil isi `new-folder-*`. ---
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.global::<SftpModel>().on_sftp_new_folder_requested(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let sftp_model = ui.global::<SftpModel>();
            if sftp_model.get_transferring() {
                return;
            }
            let pane = sftp_model.get_new_folder_pane().to_string();
            let name = sftp_model.get_new_folder_name().to_string();
            if name.trim().is_empty() {
                return;
            }
            sftp_model.set_transferring(true);
            sftp_model.set_transfer_error("".into());

            let ui_weak_task = ui_weak.clone();
            let state_task = state.clone();
            tokio::spawn(async move {
                let result: Result<(), String> = match pane.as_str() {
                    "local" => {
                        let path = state_task.sftp_local_path.lock().unwrap_or_else(|e| e.into_inner()).join(&name);
                        tokio::task::spawn_blocking(move || std::fs::create_dir(&path))
                            .await
                            .expect("blocking task panik")
                            .map_err(|e| e.to_string())
                    }
                    "remote" => {
                        let path = posix_join(&state_task.sftp_remote_path.lock().unwrap_or_else(|e| e.into_inner()), &name);
                        let sftp = state_task.sftp.lock().await;
                        match sftp.as_ref() {
                            Some(browser) => browser.create_dir(&path).await.map_err(|e| e.to_string()),
                            None => Err("sesi SFTP sudah terputus".to_string()),
                        }
                    }
                    _ => Err("panel tidak dikenal".to_string()),
                };
                let ok = result.is_ok();
                let _ = slint::invoke_from_event_loop({
                    let ui_weak_task = ui_weak_task.clone();
                    move || {
                        let Some(ui) = ui_weak_task.upgrade() else { return };
                        let sftp_model = ui.global::<SftpModel>();
                        sftp_model.set_transferring(false);
                        match result {
                            Ok(()) => sftp_model.set_new_folder_modal_open(false),
                            Err(e) => sftp_model.set_transfer_error(format!("Gagal bikin folder: {e}").into()),
                        }
                    }
                });
                if ok {
                    refresh_sftp_panes(ui_weak_task, state_task).await;
                }
            });
        });
    }
}

/// Callback seputar sesi Console (serial) — pola arsitektur MIRIP
/// `wire_terminal_callbacks` (Connect -> spawn reader yang feed
/// `TerminalInstance` -> render -> push ke Slint), tapi SATU sesi saja
/// (bukan `HashMap` per-tab, lihat komentar panjang di
/// `AppState.console_session`), jadi tidak ada logic "tab aktif".
fn wire_console_callbacks(ui: &AppWindow, state: &Arc<AppState>) {
    // --- Scan port serial yang lagi tersambung. Dipanggil waktu
    //     halaman Console dibuka (`init` di page-console.slint) DAN
    //     klik tombol ⟳ — daftar bisa berubah kapan saja (USB-to-
    //     serial adapter dicabut-colok). `list_ports()` sinkron (bukan
    //     network I/O) tapi tetap di-`spawn_blocking`, jaga-jaga kalau
    //     lambat di sistem tertentu (banyak port ter-enumerate, dst). ---
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.global::<ConsoleModel>().on_console_refresh_ports_requested(move || {
            if ui_weak.upgrade().is_none() {
                return;
            }
            let ui_weak_task = ui_weak.clone();
            let state_task = state.clone();
            tokio::spawn(async move {
                let ports = tokio::task::spawn_blocking(terminus_serial_engine::list_ports)
                    .await
                    .unwrap_or_else(|_| Ok(Vec::new()))
                    .unwrap_or_default();
                let labels: Vec<slint::SharedString> = ports.iter().map(|p| p.label.clone().into()).collect();
                *state_task.console_ports.lock().unwrap_or_else(|e| e.into_inner()) = ports;
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = ui_weak_task.upgrade() else { return };
                    let cm = ui.global::<ConsoleModel>();
                    let had_selection = cm.get_selected_port_index();
                    let n = labels.len();
                    cm.set_available_ports(ModelRc::new(VecModel::from(labels)));
                    // Pertahankan pilihan lama kalau masih valid (mis.
                    // refresh manual tanpa device berubah); default ke
                    // port PERTAMA kalau belum pernah pilih/pilihan
                    // lama sudah di luar jangkauan (device dicabut),
                    // atau -1 kalau daftar kosong (tombol Connect
                    // otomatis disabled, lihat page-console.slint).
                    let new_index = if had_selection >= 0 && (had_selection as usize) < n {
                        had_selection
                    } else if n > 0 {
                        0
                    } else {
                        -1
                    };
                    cm.set_selected_port_index(new_index);
                });
            });
        });
    }

    // --- Connect: buka port, spawn reader, mulai render grid. ---
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.global::<ConsoleModel>().on_console_connect_requested(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let cm = ui.global::<ConsoleModel>();
            if cm.get_connecting() {
                return;
            }
            let idx = cm.get_selected_port_index();
            let entry = {
                let ports = state.console_ports.lock().unwrap_or_else(|e| e.into_inner());
                if idx < 0 {
                    None
                } else {
                    ports.get(idx as usize).cloned()
                }
            };
            let Some(entry) = entry else {
                cm.set_connect_error("Pilih port dulu.".into());
                return;
            };
            let baud: u32 = cm.get_baud_rate().parse().unwrap_or(9600);
            cm.set_connecting(true);
            cm.set_connect_error("".into());
            {
                let cmm = ui.global::<ConnectingModel>();
                let avatar_letter = entry.label.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_else(|| "?".to_string());
                cmm.set_label(entry.label.clone().into());
                cmm.set_subtitle(format!("Console {baud} 8-N-1").into());
                cmm.set_avatar_letter(avatar_letter.into());
                cmm.set_visible(true);
            }

            let ui_weak_task = ui_weak.clone();
            let state_task = state.clone();
            track_connect_task(&state, None, async move {
                let result = SerialSession::connect(&entry.path, baud).await;
                let notice: Option<String> = match result {
                    // `rx` di sini SUDAH dijamin tidak ketinggalan byte
                    // awal (subscribe terjadi DI DALAM `connect()`,
                    // sebelum task pembaca mulai — lihat komentar
                    // panjang di `SerialSession::connect`), beda dari
                    // sebelumnya yang manggil `session.subscribe_
                    // output()` BELAKANGAN di sini (ada celah waktu,
                    // banner/prompt awal device bisa kelewat — akar
                    // masalah laporan user "teks tidak tampil sampai
                    // beberapa kali Enter").
                    Ok((session, rx)) => {
                        *state_task.console_session.lock().await = Some(session);
                        *state_task.console_terminal.lock().unwrap_or_else(|e| e.into_inner()) =
                            Some(TerminalInstance::new(console::TERM_COLS, console::TERM_ROWS));
                        spawn_console_reader(ui_weak_task.clone(), state_task.clone(), rx);
                        None
                    }
                    Err(e) => Some(e.to_string()),
                };
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = ui_weak_task.upgrade() else { return };
                    ui.global::<ConnectingModel>().set_visible(false);
                    let cm = ui.global::<ConsoleModel>();
                    cm.set_connecting(false);
                    match notice {
                        Some(msg) => cm.set_connect_error(msg.into()),
                        None => {
                            cm.set_device_label(entry.label.clone().into());
                            cm.set_baud_label(format!("{baud} 8-N-1").into());
                            cm.set_rows(ModelRc::new(VecModel::from(Vec::<TermRow>::new())));
                            cm.set_connected(true);
                        }
                    }
                });
            });
        });
    }

    // --- Disconnect (klik tombol — UI di-update LANGSUNG, sama pola
    //     `on_tab_close_requested`, penutupan port beneran jalan di
    //     background; device dicabut SENDIRI ditangani lewat jalur
    //     lain, lihat ujung `spawn_console_reader`). ---
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.global::<ConsoleModel>().on_console_disconnect_requested(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            finalize_console_closed(&ui);
            let state_task = state.clone();
            tokio::spawn(async move {
                if let Some(mut session) = state_task.console_session.lock().await.take() {
                    session.disconnect().await;
                }
            });
        });
    }

    // --- Input keyboard -> tulis ke port. ---
    {
        let state = state.clone();
        ui.global::<ConsoleModel>().on_console_input_requested(move |text: slint::SharedString| {
            let bytes = text.as_bytes().to_vec();
            let state = state.clone();
            tokio::spawn(async move {
                if let Some(session) = state.console_session.lock().await.as_mut() {
                    let _ = session.write(&bytes).await;
                }
            });
        });
    }
}

/// Reset tampilan Console ke status "belum connect" — dipanggil dari
/// DUA tempat: klik tombol Disconnect (langsung, UI thread) DAN ujung
/// `spawn_console_reader` (device mengirim `Closed` — dicabut/port
/// ketutup sendiri, lewat `invoke_from_event_loop`). Idempotent, aman
/// dipanggil dua kali (klik Disconnect lalu reader task ikut sampai ke
/// `Closed`-nya juga).
fn finalize_console_closed(ui: &AppWindow) {
    let cm = ui.global::<ConsoleModel>();
    cm.set_connected(false);
    cm.set_rows(ModelRc::new(VecModel::from(Vec::<TermRow>::new())));
}

/// Spawn satu task connect (SSH/SFTP/Console) DAN simpan
/// `AbortHandle`-nya ke `state.active_connect` — SATU-SATUNYA tempat
/// yang boleh `tokio::spawn` proses connect (dipanggil dari
/// `perform_connect`/`sftp_connect_and_refresh`/`on_console_connect_
/// requested`), supaya tombol "Batal" di layar `ConnectingModel` (lihat
/// `wire_connecting_callbacks` di bawah) SELALU punya cara nyata buat
/// menghentikannya. `host_id`: `Some(uuid)` KALAU DAN HANYA KALAU ini
/// SSH terminal connect (host punya status per-host yang perlu
/// di-reset ke "offline" waktu dibatalkan) — `None` buat SFTP/Console.
fn track_connect_task<F>(state: &Arc<AppState>, host_id: Option<Uuid>, fut: F)
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    let handle = tokio::spawn(fut);
    *state.active_connect.lock().unwrap_or_else(|e| e.into_inner()) = Some((host_id, handle.abort_handle()));
}

/// Callback layar "Connecting" (`ConnectingModel`, models.slint) —
/// SATU-SATUNYA yang perlu di-wire di sini cuma tombol "Batal" (isi
/// `label`/`subtitle`/`visible`-nya sendiri diisi LANGSUNG dari
/// `perform_connect`/`sftp_connect_and_refresh`/`on_console_connect_
/// requested`, bukan lewat callback terpisah).
fn wire_connecting_callbacks(ui: &AppWindow, state: &Arc<AppState>) {
    let ui_weak = ui.as_weak();
    let state = state.clone();
    ui.global::<ConnectingModel>().on_cancel_requested(move || {
        let Some(ui) = ui_weak.upgrade() else { return };
        let prev = state.active_connect.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some((host_id, handle)) = prev {
            handle.abort();
            if let Some(uuid) = host_id {
                set_status(&state, uuid, "offline");
                refresh_hosts_model(&ui, &state);
            }
        }
        ui.global::<ConnectingModel>().set_visible(false);
        ui.global::<SftpModel>().set_connecting(false);
        ui.global::<ConsoleModel>().set_connecting(false);
    });
}

/// Task yang hidup selama satu sesi Console berlangsung — pola SAMA
/// dengan `spawn_terminal_reader` (batching biar output yang datang
/// beruntun kecil-kecil tidak memicu render per-potongan), TAPI lebih
/// sederhana: tidak ada konsep "tab aktif" (Console cuma satu sesi
/// dalam satu waktu), jadi SELALU dorong ke `ConsoleModel.rows` begitu
/// ada data baru, tidak perlu cache per-id kayak `tab_cache`.
fn spawn_console_reader(ui_weak: slint::Weak<AppWindow>, state: Arc<AppState>, mut rx: broadcast::Receiver<SerialOutputEvent>) {
    const IDLE_GAP: Duration = Duration::from_millis(5);
    const MAX_BATCH: Duration = Duration::from_millis(40);

    tokio::spawn(async move {
        let feed = |state: &Arc<AppState>, bytes: &[u8]| {
            if let Some(term) = state.console_terminal.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
                term.feed(bytes);
            }
        };

        'session: loop {
            match rx.recv().await {
                Ok(SerialOutputEvent::Data(bytes)) => feed(&state, &bytes),
                Ok(SerialOutputEvent::Closed) => break 'session,
                Err(broadcast::error::RecvError::Closed) => break 'session,
                Err(broadcast::error::RecvError::Lagged(_)) => continue 'session,
            }

            let batch_deadline = tokio::time::Instant::now() + MAX_BATCH;
            let mut closed = false;
            loop {
                let remaining = batch_deadline.saturating_duration_since(tokio::time::Instant::now());
                if remaining.is_zero() {
                    break;
                }
                match tokio::time::timeout(remaining.min(IDLE_GAP), rx.recv()).await {
                    Ok(Ok(SerialOutputEvent::Data(more))) => feed(&state, &more),
                    Ok(Ok(SerialOutputEvent::Closed)) => {
                        closed = true;
                        break;
                    }
                    Ok(Err(broadcast::error::RecvError::Closed)) => {
                        closed = true;
                        break;
                    }
                    Ok(Err(broadcast::error::RecvError::Lagged(_))) => continue,
                    Err(_) => break, // timeout: idle, tidak ada lagi buat sementara
                }
            }

            render_and_push_console(&ui_weak, &state);

            if closed {
                break 'session;
            }
        }

        *state.console_session.lock().await = None;
        *state.console_terminal.lock().unwrap_or_else(|e| e.into_inner()) = None;
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = ui_weak.upgrade() {
                finalize_console_closed(&ui);
            }
        });
    });
}

/// Snapshot `state.console_terminal` (pakai color theme yang lagi
/// aktif — SAMA tema yang dipakai tab SSH, `TerminalTabsModel`
/// bukanlah "punya" Console, tapi tema terminal memang pengaturan
/// GLOBAL, lihat komentar `AppState.terminal_theme`) dan dorong ke
/// `ConsoleModel.rows`.
fn render_and_push_console(ui_weak: &slint::Weak<AppWindow>, state: &Arc<AppState>) {
    let plain = {
        let terminal = state.console_terminal.lock().unwrap_or_else(|e| e.into_inner());
        let Some(term) = terminal.as_ref() else { return };
        let theme = state.terminal_theme.lock().unwrap_or_else(|e| e.into_inner());
        console::grid_to_plain_rows(&term.snapshot(&theme))
    };
    let ui_weak2 = ui_weak.clone();
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(ui) = ui_weak2.upgrade() {
            ui.global::<ConsoleModel>().set_rows(console::plain_rows_to_slint(plain));
        }
    });
}

/// Connect (saved ATAU manual, dua-duanya lewat sini) lalu langsung
/// list isi home directory-nya (remote) + path lokal yang lagi
/// tersimpan (biasanya `$HOME`) — biar begitu browser kebuka user
/// LANGSUNG lihat isi, bukan dua panel kosong nunggu klik refresh.
async fn sftp_connect_and_refresh(
    ui_weak: slint::Weak<AppWindow>,
    state: Arc<AppState>,
    profile: HostProfile,
    secret: SecretMaterial,
) {
    let host_key_store: Arc<dyn HostKeyStore> = Arc::new(AppHostKeyStore::new(state.vault.clone()));
    let label = profile.label.clone();
    {
        let subtitle = format!("SFTP {}:{}", profile.host, profile.port);
        let avatar_letter = label.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_else(|| "?".to_string());
        let label = label.clone();
        let ui_weak = ui_weak.clone();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = ui_weak.upgrade() {
                let cm = ui.global::<ConnectingModel>();
                cm.set_label(label.into());
                cm.set_subtitle(subtitle.into());
                cm.set_avatar_letter(avatar_letter.into());
                cm.set_visible(true);
            }
        });
    }
    let result = SftpBrowser::connect(&profile, secret, host_key_store).await;

    let outcome: Result<String, SftpError> = match result {
        Ok(browser) => {
            let home = browser.home_dir().await.unwrap_or_else(|_| "/".to_string());
            *state.sftp.lock().await = Some(browser);
            *state.sftp_remote_path.lock().unwrap_or_else(|e| e.into_inner()) = home.clone();
            Ok(home)
        }
        Err(e) => Err(e),
    };

    match outcome {
        Ok(_) => {
            let ui_weak2 = ui_weak.clone();
            let _ = slint::invoke_from_event_loop(move || {
                let Some(ui) = ui_weak2.upgrade() else { return };
                ui.global::<ConnectingModel>().set_visible(false);
                let sftp_model = ui.global::<SftpModel>();
                sftp_model.set_connecting(false);
                sftp_model.set_connected(true);
                // Modal manual connect (kalau lagi kebuka) HARUS ditutup
                // di sini — beda dari slide-over sebelumnya, modal
                // digambar TERPISAH dari kondisi `connected`, jadi kalau
                // tidak ditutup eksplisit dia bakal nutupin browser
                // two-pane yang baru muncul.
                sftp_model.set_manual_modal_open(false);
                sftp_model.set_remote_host_label(label.into());
            });
            refresh_sftp_panes(ui_weak, state).await;
        }
        Err(e) => {
            let _ = slint::invoke_from_event_loop(move || {
                let Some(ui) = ui_weak.upgrade() else { return };
                ui.global::<ConnectingModel>().set_visible(false);
                let sftp_model = ui.global::<SftpModel>();
                sftp_model.set_connecting(false);
                sftp_model.set_connect_error(e.to_string().into());
            });
        }
    }
}

/// List ULANG dua panel (local + remote) di path yang lagi tersimpan
/// di `state`, dorong hasilnya ke `SftpModel`. Dipakai dari connect
/// awal, refresh, DAN tiap navigasi (klik dua kali folder) — satu
/// fungsi biar tidak duplikasi logic konversi entry -> `FileEntry`.
async fn refresh_sftp_panes(ui_weak: slint::Weak<AppWindow>, state: Arc<AppState>) {
    let local_path = state.sftp_local_path.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let remote_path = state.sftp_remote_path.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let local_show_hidden = *state.sftp_local_show_hidden.lock().unwrap_or_else(|e| e.into_inner());
    let remote_show_hidden = *state.sftp_remote_show_hidden.lock().unwrap_or_else(|e| e.into_inner());

    let local_path_for_blocking = local_path.clone();
    let local_entries = tokio::task::spawn_blocking(move || list_local_dir(&local_path_for_blocking))
        .await
        .unwrap_or_default();

    let remote_entries = {
        let sftp = state.sftp.lock().await;
        match sftp.as_ref() {
            Some(browser) => browser.list_dir(&remote_path).await.unwrap_or_default(),
            None => Vec::new(),
        }
    };

    let local_at_root = local_path.parent().is_none();
    let remote_at_root = remote_path == "/";
    let local_path_str = local_path.to_string_lossy().to_string();
    let local_selected = state.sftp_local_selected.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let remote_selected = state.sftp_remote_selected.lock().unwrap_or_else(|e| e.into_inner()).clone();

    let _ = slint::invoke_from_event_loop(move || {
        let Some(ui) = ui_weak.upgrade() else { return };
        let sftp_model = ui.global::<SftpModel>();
        sftp_model.set_local_path(local_path_str.into());
        sftp_model.set_remote_path(remote_path.into());
        sftp_model.set_local_entries(ModelRc::new(VecModel::from(local_entries_to_slint(
            local_entries,
            local_at_root,
            local_show_hidden,
            &local_selected,
        ))));
        sftp_model.set_remote_entries(ModelRc::new(VecModel::from(remote_entries_to_slint(
            remote_entries,
            remote_at_root,
            remote_show_hidden,
            &remote_selected,
        ))));
    });
}

/// List satu direktori LOKAL beneran (`std::fs::read_dir`, BUKAN
/// mock) — folder dulu (alfabetis), baru file (alfabetis), sama gaya
/// pengurutan dengan `SftpBrowser::list_dir` (remote). Dipanggil dari
/// dalam `spawn_blocking` (I/O sinkron) — JANGAN panggil langsung dari
/// task async biasa. Dotfile TIDAK difilter di sini — filternya di
/// `local_entries_to_slint` (satu tempat, dipakai local MAUPUN remote).
fn list_local_dir(path: &std::path::Path) -> Vec<(String, bool, u64, Option<i64>)> {
    let mut out = Vec::new();
    let Ok(read_dir) = std::fs::read_dir(path) else { return out };
    for entry in read_dir.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let Ok(meta) = entry.metadata() else { continue };
        let modified = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64);
        out.push((name, meta.is_dir(), meta.len(), modified));
    }
    out.sort_by(|a, b| match (a.1, b.1) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.0.to_lowercase().cmp(&b.0.to_lowercase()),
    });
    out
}

/// Sisipkan baris semu ".." (kalau BUKAN di root), filter dotfile
/// (kecuali `show_hidden`), + format tiap entry jadi `FileEntry`
/// (struct Slint, termasuk `checked` dari `selected` — set nama file
/// yang lagi dicentang buat "Select All"/bulk Delete di menu select
/// option) — dipakai buat local MAUPUN remote (lihat dua fungsi di
/// bawah), ditarik ke satu tempat biar konsisten dua-duanya.
fn local_entries_to_slint(
    entries: Vec<(String, bool, u64, Option<i64>)>,
    at_root: bool,
    show_hidden: bool,
    selected: &std::collections::HashSet<String>,
) -> Vec<FileEntry> {
    let mut out = Vec::new();
    if !at_root {
        out.push(parent_row());
    }
    out.extend(
        entries
            .into_iter()
            .filter(|(name, ..)| show_hidden || !name.starts_with('.'))
            .map(|(name, is_dir, size, modified)| FileEntry {
                checked: selected.contains(&name),
                name: name.into(),
                is_dir,
                size: if is_dir { "--".into() } else { format_size(size).into() },
                modified: modified.map(format_epoch_date).unwrap_or_default().into(),
            }),
    );
    out
}

fn remote_entries_to_slint(
    entries: Vec<RemoteEntry>,
    at_root: bool,
    show_hidden: bool,
    selected: &std::collections::HashSet<String>,
) -> Vec<FileEntry> {
    let mut out = Vec::new();
    if !at_root {
        out.push(parent_row());
    }
    out.extend(entries.into_iter().filter(|e| show_hidden || !e.name.starts_with('.')).map(|e| FileEntry {
        checked: selected.contains(&e.name),
        name: e.name.into(),
        is_dir: e.is_dir,
        size: if e.is_dir { "--".into() } else { format_size(e.size).into() },
        modified: e.modified.map(format_epoch_date).unwrap_or_default().into(),
    }));
    out
}

/// Baris semu ".." — nama SENGAJA persis ini, dicek balik di handler
/// `sftp-*-navigate-requested` (lihat `wire_sftp_callbacks`) buat tahu
/// "ini permintaan naik satu level", bukan masuk folder beneran
/// bernama titik-dua.
fn parent_row() -> FileEntry {
    FileEntry { name: "..".into(), is_dir: true, size: "".into(), modified: "".into(), checked: false }
}

/// Induk dari path POSIX (SFTP SELALU "/", terlepas OS server-nya) —
/// `std::path::Path` cocoknya buat path OS LOKAL, bukan ini, makanya
/// ditulis manual sebagai manipulasi string.
fn posix_parent(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    match trimmed.rfind('/') {
        Some(0) => "/".to_string(),
        Some(idx) => trimmed[..idx].to_string(),
        None => "/".to_string(),
    }
}

fn posix_join(path: &str, name: &str) -> String {
    if path == "/" {
        format!("/{name}")
    } else {
        format!("{}/{name}", path.trim_end_matches('/'))
    }
}

fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    format!("{size:.1} {}", UNITS[unit])
}

/// Format detik unix epoch jadi "YYYY-MM-DD" TANPA dependency date
/// eksternal baru — konversi hari-sejak-epoch ke tanggal kalender
/// pakai algoritma `civil_from_days` (Howard Hinnant, proleptic
/// Gregorian, sudah dipakai luas & dites di banyak implementasi date
/// minimal seperti ini).
fn format_epoch_date(epoch_secs: i64) -> String {
    let days = epoch_secs.div_euclid(86400);
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}")
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

/// Pindahkan tampilan ke tab milik `id` (yang HARUS sudah ada di
/// `state.sessions`/`tab_meta`) — dipakai setelah connect baru sukses,
/// klik tab di tab bar, MAUPUN waktu user klik Connect ke host yang
/// sesinya sudah berjalan di background. Isi `rows` langsung dari cache
/// terakhir (`tab_cache`) kalau ada, supaya tidak blank sampai ada byte
/// baru datang.
fn switch_to_tab(ui: &AppWindow, state: &Arc<AppState>, id: Uuid) {
    *state.active_terminal.lock().unwrap_or_else(|e| e.into_inner()) = Some(id);

    let (label, address) = state
        .tab_meta
        .lock()
        .unwrap()
        .iter()
        .find(|t| t.id == id)
        .map(|t| (t.label.clone(), t.address.clone()))
        .unwrap_or_default();

    let cached_rows = state.tab_cache.lock().unwrap_or_else(|e| e.into_inner()).get(&id).cloned();

    let tm = ui.global::<TerminalTabsModel>();
    tm.set_active_tab_id(id.to_string().into());
    tm.set_active_tab_label(label.into());
    tm.set_active_tab_address(address.into());
    tm.set_rows(match cached_rows {
        Some(rows) => console::plain_rows_to_slint(rows),
        None => ModelRc::new(VecModel::from(Vec::<TermRow>::new())),
    });
    ui.set_current_page(6); // 6 = halaman virtual Terminal, lihat app-window.slint
}

/// Bersih-bersih state SISI UI setelah satu tab beneran ketutup — baik
/// karena user klik "×" MAUPUN karena remote nutup channel duluan
/// (lihat `spawn_terminal_reader`). TIDAK mengurus penutupan
/// `SshSession` itu sendiri (caller yang tanggung jawab itu, kalau
/// perlu — waktu dipanggil dari reader task, sesinya sudah otomatis
/// mati di sisi remote).
fn finalize_tab_closed(ui: &AppWindow, state: &Arc<AppState>, id: Uuid) {
    state.tab_cache.lock().unwrap_or_else(|e| e.into_inner()).remove(&id);
    // Kalau dipanggil dari `on_tab_close_requested` (klik "×"), reader
    // task punya `TerminalInstance` ini masih hidup sampai loop-nya
    // sendiri berhenti (disconnect network butuh waktu) — hapus dari
    // sini juga (idempotent, aman dipanggil dua kali) biar tidak ada
    // entry menggantung selama itu.
    state.terminals.lock().unwrap_or_else(|e| e.into_inner()).remove(&id);
    let next_tab = {
        let mut meta = state.tab_meta.lock().unwrap_or_else(|e| e.into_inner());
        meta.retain(|t| t.id != id);
        meta.first().map(|t| t.id)
    };
    refresh_terminal_tabs_model(ui, state);
    set_status(state, id, "offline");

    let was_active = *state.active_terminal.lock().unwrap_or_else(|e| e.into_inner()) == Some(id);
    if was_active {
        match next_tab {
            Some(next_id) => switch_to_tab(ui, state, next_id),
            None => {
                *state.active_terminal.lock().unwrap_or_else(|e| e.into_inner()) = None;
                let tm = ui.global::<TerminalTabsModel>();
                tm.set_active_tab_id("".into());
                tm.set_active_tab_label("".into());
                tm.set_active_tab_address("".into());
                tm.set_rows(ModelRc::new(VecModel::from(Vec::<TermRow>::new())));
                ui.set_current_page(0); // balik ke Hosts, tidak ada tab lagi buat ditampilkan
            }
        }
    }
    refresh_hosts_model(ui, state);
}

/// Baca ulang `state.tab_meta` dan tulis ke `TerminalTabsModel.tabs`.
fn refresh_terminal_tabs_model(ui: &AppWindow, state: &Arc<AppState>) {
    let items: Vec<TerminalTab> = state
        .tab_meta
        .lock()
        .unwrap()
        .iter()
        .map(|t| TerminalTab { id: t.id.to_string().into(), label: t.label.clone().into(), address: t.address.clone().into() })
        .collect();
    ui.global::<TerminalTabsModel>().set_tabs(ModelRc::new(VecModel::from(items)));
}

/// Jalankan proses connect SSH beneran: set status "connecting", buka
/// koneksi, kalau sukses buka tab terminal baru (spawn reader + pindah
/// tampilan ke situ), kalau gagal tampilkan notice error. `profile` +
/// `password` dioper LANGSUNG sebagai argumen (bukan dibaca ulang dari
/// vault di sini) — dipakai dari DUA tempat: host yang SUDAH tersimpan
/// (`on_host_connect_requested`, baca dari vault dulu) MAUPUN dari
/// slide-over mode "New Host" yang baru saja disimpan
/// (`on_host_connect_new_requested`, profile-nya masih "hangat" di
/// tangan, tidak perlu baca ulang).
async fn perform_connect(ui_weak: slint::Weak<AppWindow>, state: Arc<AppState>, profile: HostProfile, password: String) {
    let uuid = profile.id;
    set_status(&state, uuid, "connecting");
    {
        let ui_weak = ui_weak.clone();
        let state = state.clone();
        let label = profile.label.clone();
        let subtitle = format!("SSH {}:{}", profile.host, profile.port);
        let avatar_letter = label.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_else(|| "?".to_string());
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = ui_weak.upgrade() {
                refresh_hosts_model(&ui, &state);
                let cm = ui.global::<ConnectingModel>();
                cm.set_label(label.into());
                cm.set_subtitle(subtitle.into());
                cm.set_avatar_letter(avatar_letter.into());
                cm.set_visible(true);
            }
        });
    }

    let host_key_store: Arc<dyn HostKeyStore> = Arc::new(AppHostKeyStore::new(state.vault.clone()));
    let result = terminus_ssh_engine::connect(&profile, SecretMaterial::Password(password), host_key_store).await;

    let notice: Option<String> = match result {
        Ok(session) => {
            // Selaraskan ukuran PTY server dengan grid `TerminalInstance`
            // di bawah (lihat `console::TERM_COLS/ROWS`) — best-effort,
            // gagal resize TIDAK menggagalkan koneksi.
            let _ = session.resize_pty(console::TERM_COLS, console::TERM_ROWS).await;
            let rx = session.subscribe_output();
            state.sessions.lock().await.insert(uuid, session);
            set_status(&state, uuid, "online");
            state.tab_meta.lock().unwrap_or_else(|e| e.into_inner()).push(TabMeta {
                id: uuid,
                label: profile.label.clone(),
                address: format!("{}:{}", profile.host, profile.port),
            });
            spawn_terminal_reader(ui_weak.clone(), state.clone(), uuid, rx);
            None
        }
        Err(e) => {
            set_status(&state, uuid, "failed");
            Some(e.to_string())
        }
    };

    let _ = slint::invoke_from_event_loop(move || {
        if let Some(ui) = ui_weak.upgrade() {
            ui.global::<ConnectingModel>().set_visible(false);
            match notice {
                Some(msg) => show_notice(&ui, &msg, true),
                None => {
                    refresh_terminal_tabs_model(&ui, &state);
                    switch_to_tab(&ui, &state, uuid);
                }
            }
            refresh_hosts_model(&ui, &state);
        }
    });
}

/// Task yang hidup selama satu sesi SSH berlangsung: feed byte ke
/// `TerminalInstance` (parser VTE) MILIK tab ini di `state.terminals`
/// — SENGAJA di situ (bukan variabel lokal task ini lagi) supaya bisa
/// di-snapshot ULANG dari LUAR (`on_terminal_theme_changed`) waktu
/// user ganti color theme, tanpa perlu nunggu byte baru dari SSH.
///
/// Debounce: byte dari SSH sering datang beruntun kecil-kecil (echo per
/// keystroke, output command yang kepecah beberapa paket TCP). Kalau
/// tiap potongan langsung memicu render ulang (rebuild ~30 baris
/// elemen Slint + `invoke_from_event_loop` + repaint), itu berat &
/// bikin terasa lambat waktu ngetik/lihat output panjang. Jadi: begitu
/// satu potongan pertama datang, tunggu SEBENTAR (maks 5ms idle,
/// maks total 40ms per batch) buat "nyerok" potongan susulan yang
/// kemungkinan besar datang hampir bersamaan, feed semuanya ke
/// `TerminalInstance`, BARU render sekali. Batas 40ms mencegah output
/// yang mengalir TERUS-MENERUS (mis. `cat` file besar) menahan render
/// selamanya — tetap update ~25x/detik minimal.
fn spawn_terminal_reader(
    ui_weak: slint::Weak<AppWindow>,
    state: Arc<AppState>,
    host_id: Uuid,
    mut rx: broadcast::Receiver<SshOutputEvent>,
) {
    const IDLE_GAP: Duration = Duration::from_millis(5);
    const MAX_BATCH: Duration = Duration::from_millis(40);

    state.terminals.lock().unwrap_or_else(|e| e.into_inner()).insert(host_id, TerminalInstance::new(console::TERM_COLS, console::TERM_ROWS));

    tokio::spawn(async move {
        // Kunci `state.terminals` cuma sebentar per panggilan `feed`
        // SENGAJA tidak dipegang lintas `.await` (bakal bikin future
        // ini non-`Send`, ditolak `tokio::spawn`) — makanya lock-nya
        // dibuka-tutup tiap potongan, bukan sekali di awal batch.
        let feed = |state: &Arc<AppState>, bytes: &[u8]| {
            if let Some(term) = state.terminals.lock().unwrap_or_else(|e| e.into_inner()).get_mut(&host_id) {
                term.feed(bytes);
            }
        };

        'session: loop {
            // Tunggu potongan PERTAMA dari batch ini (boleh nunggu lama
            // — tidak ada apa-apa buat dirender sampai ada data).
            match rx.recv().await {
                Ok(SshOutputEvent::Data(bytes)) => feed(&state, &bytes),
                Ok(SshOutputEvent::Closed) => break 'session,
                Err(broadcast::error::RecvError::Closed) => break 'session,
                Err(broadcast::error::RecvError::Lagged(_)) => continue 'session,
            }

            // Serok potongan susulan (idle 5ms ATAU 40ms total, mana
            // yang lebih dulu tercapai) jadi SATU render.
            let batch_deadline = tokio::time::Instant::now() + MAX_BATCH;
            let mut closed = false;
            loop {
                let remaining = batch_deadline.saturating_duration_since(tokio::time::Instant::now());
                if remaining.is_zero() {
                    break;
                }
                match tokio::time::timeout(remaining.min(IDLE_GAP), rx.recv()).await {
                    Ok(Ok(SshOutputEvent::Data(more))) => feed(&state, &more),
                    Ok(Ok(SshOutputEvent::Closed)) => {
                        closed = true;
                        break;
                    }
                    Ok(Err(broadcast::error::RecvError::Closed)) => {
                        closed = true;
                        break;
                    }
                    Ok(Err(broadcast::error::RecvError::Lagged(_))) => continue,
                    Err(_) => break, // timeout: idle, tidak ada lagi buat sementara
                }
            }

            // Simpan snapshot terbaru ke cache SELALU (biar pindah tab
            // ke sesi ini nanti langsung nampilin isi terkini), tapi
            // cuma dorong ke UI kalau tab ini yang lagi ditampilkan.
            render_and_cache_tab(&ui_weak, &state, host_id);

            if closed {
                break 'session;
            }
        }

        // Sesi beneran berakhir (remote nutup channel, mis. user
        // ngetik `exit` — bukan lewat klik "×" tab kita). Bersihkan
        // sesi & tab-nya otomatis.
        state.sessions.lock().await.remove(&host_id);
        state.terminals.lock().unwrap_or_else(|e| e.into_inner()).remove(&host_id);
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = ui_weak.upgrade() {
                finalize_tab_closed(&ui, &state, host_id);
            }
        });
    });
}

/// Snapshot `TerminalInstance` milik `host_id` (pakai color theme yang
/// LAGI aktif), simpan ke `tab_cache`, dan dorong ke `TerminalTabsModel
/// .rows` KALAU tab ini yang lagi ditampilkan. Dipakai dari dua
/// tempat: `spawn_terminal_reader` (ada byte baru) DAN
/// `on_terminal_theme_changed` (tema ganti, konten SAMA tapi warnanya
/// harus di-resolve ulang).
fn render_and_cache_tab(ui_weak: &slint::Weak<AppWindow>, state: &Arc<AppState>, host_id: Uuid) {
    let plain = {
        let terminals = state.terminals.lock().unwrap_or_else(|e| e.into_inner());
        let Some(term) = terminals.get(&host_id) else { return };
        let theme = state.terminal_theme.lock().unwrap_or_else(|e| e.into_inner());
        console::grid_to_plain_rows(&term.snapshot(&theme))
    };
    state.tab_cache.lock().unwrap_or_else(|e| e.into_inner()).insert(host_id, plain.clone());
    let is_active = *state.active_terminal.lock().unwrap_or_else(|e| e.into_inner()) == Some(host_id);
    if is_active {
        let ui_weak2 = ui_weak.clone();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = ui_weak2.upgrade() {
                ui.global::<TerminalTabsModel>().set_rows(console::plain_rows_to_slint(plain));
            }
        });
    }
}

/// Scan font MONOSPACE yang beneran terinstall di sistem (lewat
/// `fontdb` — sudah jadi dependency transitif Slint sendiri buat
/// render font, dipakai ulang di sini, bukan nambah crate baru).
/// Dipanggil SEKALI waktu startup (lihat `wire_callbacks`), bukan
/// setiap kali panel pengaturan terminal dibuka — daftar font
/// terinstall tidak berubah selama app jalan.
///
/// `FaceInfo` satu per FACE (Regular/Bold/Italic dst masing-masing
/// entry terpisah) — makanya di-dedup+sort lewat `BTreeSet`, biar
/// "JetBrains Mono" cuma muncul SEKALI di daftar meski py fontdb
/// nemuin banyak variannya.
fn scan_monospace_fonts() -> Vec<slint::SharedString> {
    let mut db = fontdb::Database::new();
    db.load_system_fonts();

    let names: std::collections::BTreeSet<String> = db
        .faces()
        .filter(|face| face.monospaced)
        .filter_map(|face| face.families.first().map(|(name, _)| name.clone()))
        .collect();

    if names.is_empty() {
        // Sistem aneh yang fontdb tidak nemuin apa-apa (jarang terjadi)
        // — tetap kasih beberapa opsi generik yang biasanya ada,
        // daripada daftar kosong sama sekali.
        return vec!["monospace".into(), "DejaVu Sans Mono".into(), "Courier New".into()];
    }
    names.into_iter().map(slint::SharedString::from).collect()
}

fn set_status(state: &Arc<AppState>, id: Uuid, status: &str) {
    state.statuses.lock().unwrap().insert(id, status.to_string());
}

fn status_of(state: &Arc<AppState>, id: Uuid) -> String {
    state.statuses.lock().unwrap().get(&id).cloned().unwrap_or_else(|| "offline".into())
}

fn show_notice(ui: &AppWindow, message: &str, is_error: bool) {
    ui.global::<HostsModel>().set_notice_message(message.into());
    ui.global::<HostsModel>().set_notice_is_error(is_error);
}

/// "production, database" -> ["production", "database"] — dipisah
/// koma, trim spasi, buang entry kosong (mis. koma dobel/trailing).
fn parse_tags(raw: &str) -> Vec<String> {
    raw.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
}

/// Isi `HostsModel.panel-host-*` dari satu `HostProfile` — dipakai
/// tiap kali panel "Host Details" harus nampilin/refresh data host
/// tertentu (klik kartu, setelah simpan, setelah duplicate).
/// SENGAJA tidak pernah mengisi field password — itu tetap terenkripsi
/// & tidak pernah ditampilkan balik ke UI dalam bentuk plaintext.
fn populate_panel(ui: &AppWindow, state: &Arc<AppState>, profile: &HostProfile) {
    let hm = ui.global::<HostsModel>();
    hm.set_panel_host_id(profile.id.to_string().into());
    hm.set_panel_host_label(profile.label.clone().into());
    hm.set_panel_host_host(profile.host.clone().into());
    hm.set_panel_host_port(profile.port as i32);
    hm.set_panel_host_username(profile.username.clone().into());
    hm.set_panel_host_kind(
        match profile.kind {
            ConnectionKind::Ssh => "ssh",
            ConnectionKind::CiscoIos => "cisco_ios",
        }
        .into(),
    );
    hm.set_panel_host_group_id(profile.group_id.map(|g| g.to_string()).unwrap_or_default().into());
    hm.set_panel_host_tags(profile.tags.join(", ").into());
    hm.set_panel_host_status(status_of(state, profile.id).into());
    // Defensif: profil ASLI selalu berarti mode edit, bukan create.
    // Relevan waktu dipanggil setelah `create-host-requested` sukses
    // (slide-over "berubah" dari mode New Host jadi edit host itu).
    hm.set_panel_is_new(false);
}

/// `query` HARUS sudah lowercase (dipanggil sekali di caller, bukan di
/// tiap iterasi). Cocok kalau muncul di label, host/IP, username,
/// gabungan "username@host" (biar cari "root@10.0.0.1" langsung
/// kena), ATAU salah satu tags — mencakup host ungrouped MAUPUN yang
/// ada di dalam grup manapun karena caller baca dari
/// `list_all_profiles()`, bukan dari list yang sudah difilter grup.
fn host_matches_query(p: &HostProfile, query: &str) -> bool {
    p.label.to_lowercase().contains(query)
        || p.host.to_lowercase().contains(query)
        || p.username.to_lowercase().contains(query)
        || format!("{}@{}", p.username, p.host).to_lowercase().contains(query)
        || p.tags.iter().any(|t| t.to_lowercase().contains(query))
}

fn host_profile_to_item(state: &Arc<AppState>, p: &HostProfile) -> HostItem {
    HostItem {
        id: p.id.to_string().into(),
        label: p.label.clone().into(),
        address: format!("{}:{}", p.host, p.port).into(),
        kind: match p.kind {
            ConnectionKind::Ssh => "SSH".into(),
            ConnectionKind::CiscoIos => "IOS".into(),
        },
        tags: p.tags.join(", ").into(),
        status: status_of(state, p.id).into(),
        group: p.group_id.map(|g| g.to_string()).unwrap_or_default().into(),
        checked: state.selected_hosts.lock().unwrap().contains(&p.id),
    }
}

/// Baca ulang semua data dari vault dan tulis ke `HostsModel` global.
/// Dipanggil setelah SETIAP mutasi (create/delete host & grup, hasil
/// connect) — pendekatan "rebuild semua" sengaja dipilih daripada
/// mutasi parsial per-item, lebih gampang dijaga konsistensinya untuk
/// ukuran data yang wajar (daftar host personal, bukan ribuan baris).
fn refresh_hosts_model(ui: &AppWindow, state: &Arc<AppState>) {
    let (ungrouped, groups, all) = {
        let vault = state.vault.lock().unwrap();
        (
            vault.list_ungrouped_profiles().unwrap_or_default(),
            vault.list_groups().unwrap_or_default(),
            vault.list_all_profiles().unwrap_or_default(),
        )
    };

    let ungrouped_items: Vec<HostItem> = ungrouped.iter().map(|p| host_profile_to_item(state, p)).collect();
    ui.global::<HostsModel>().set_ungrouped_hosts(ModelRc::new(VecModel::from(ungrouped_items)));

    let group_items: Vec<GroupItem> = groups
        .iter()
        .map(|g| {
            let members: Vec<&HostProfile> = all.iter().filter(|p| p.group_id == Some(g.id)).collect();
            let online = members.iter().filter(|p| status_of(state, p.id) == "online").count() as i32;
            GroupItem {
                id: g.id.to_string().into(),
                name: g.name.clone().into(),
                subtitle: g.subtitle.clone().unwrap_or_default().into(),
                online_count: online,
                total_count: members.len() as i32,
                accent: slint::Color::from_rgb_u8(0x4d, 0x8e, 0xff),
                checked: state.selected_groups.lock().unwrap().contains(&g.id),
            }
        })
        .collect();
    ui.global::<HostsModel>().set_groups(ModelRc::new(VecModel::from(group_items)));

    let selected_group_id = ui.global::<HostsModel>().get_selected_group_id().to_string();
    if !selected_group_id.is_empty() {
        if let Ok(uuid) = Uuid::parse_str(&selected_group_id) {
            let members: Vec<HostItem> = all
                .iter()
                .filter(|p| p.group_id == Some(uuid))
                .map(|p| host_profile_to_item(state, p))
                .collect();
            ui.global::<HostsModel>().set_active_group_hosts(ModelRc::new(VecModel::from(members)));
        }
    }
}

#[cfg(test)]
mod tests {
    //! Test ini menjalankan `wire_callbacks` BENERAN (bukan mock) di atas
    //! `AppWindow` beneran + `VaultStore` file sementara, lalu men-
    //! simulasikan klik tombol UI dengan `invoke_<callback>(...)` —
    //! persis apa yang Slint panggil waktu tombol beneran diklik.
    //!
    //! SEMUA operasi vault sekarang beneran async (`tokio::spawn` +
    //! `spawn_blocking`, lihat komentar di atas modul ini) — supaya
    //! hasilnya bisa diobservasi test ini, event loop Slint yang
    //! SUNGGUHAN dijalankan (`slint::run_event_loop_until_quit()`) di
    //! dalam satu `tokio::runtime::Runtime` yang di-`enter()` (PERSIS
    //! pola yang dipakai `main.rs`), bukan cuma `AppWindow::new()` diam
    //! tanpa loop jalan (yang bikin `invoke_from_event_loop` TIDAK
    //! PERNAH benar-benar dieksekusi — dibuktikan lewat probe manual
    //! sebelum kode ini ditulis: `slint::invoke_from_event_loop` yang
    //! dipanggil dari thread lain tidak pernah kelihatan efeknya kalau
    //! tidak ada `ui.run()`/`run_event_loop_until_quit()` yang jalan).
    use super::*;
    use crate::NewHostForm;
    use slint::Model;
    use std::time::Duration as StdDuration;

    /// Bukti nyata (bukan cuma "compile") `fontdb` beneran nemuin font
    /// asli di mesin ini — bukan mock/daftar hardcode. Mesin dev tanpa
    /// font monospace apa pun ITU SENDIRI sudah aneh (hampir mustahil
    /// buat sistem desktop Linux/macOS), jadi list KOSONG di sini lebih
    /// mengindikasikan bug di `scan_monospace_fonts` daripada mesin
    /// yang genuinely tanpa font.
    #[test]
    fn scan_monospace_fonts_nemuin_font_asli_di_mesin_ini() {
        let fonts = scan_monospace_fonts();
        assert!(!fonts.is_empty(), "harus nemuin minimal satu font monospace di sistem");
        println!("Font monospace terdeteksi: {fonts:?}");
    }

    fn temp_vault() -> VaultStore {
        let path = std::env::temp_dir().join(format!("terminus-app-test-{}.db", Uuid::new_v4()));
        VaultStore::open_at(path).unwrap()
    }

    /// Sample XML SecureCRT kecil: dua host share satu folder path
    /// bertingkat (buat nguji dedup grup), satu host di root (ungrouped),
    /// satu sesi Serial (harus kehitung skipped, bukan diimpor).
    const IMPORT_SAMPLE_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
    <VanDyke version="3.0">
        <key name="Sessions">
            <key name="ROUTER">
                <key name="BAROKAH">
                    <key name="rtr-01">
                        <dword name="Is Session">1</dword>
                        <string name="Protocol Name">SSH2</string>
                        <string name="Hostname">10.1.1.1</string>
                        <string name="Username">admin</string>
                        <dword name="[SSH2] Port">22</dword>
                    </key>
                    <key name="rtr-02">
                        <dword name="Is Session">1</dword>
                        <string name="Protocol Name">SSH2</string>
                        <string name="Hostname">10.1.1.2</string>
                        <string name="Username">admin</string>
                        <dword name="[SSH2] Port">2222</dword>
                    </key>
                </key>
            </key>
            <key name="standalone-host">
                <dword name="Is Session">1</dword>
                <string name="Protocol Name">SSH2</string>
                <string name="Hostname">10.2.2.2</string>
                <string name="Username">root</string>
                <dword name="[SSH2] Port">22</dword>
            </key>
            <key name="console-port">
                <dword name="Is Session">1</dword>
                <string name="Protocol Name">Serial</string>
            </key>
        </key>
    </VanDyke>"#;

    /// Membuktikan `import_parsed_hosts_into_vault` (dipakai
    /// `on_import_xml_requested`) beneran: (1) host di folder
    /// BERTINGKAT dapat SATU grup gabungan ("ROUTER / BAROKAH"), bukan
    /// dua grup terpisah atau nested; (2) host di root Sessions tetap
    /// ungrouped; (3) sesi non-SSH2 KEHITUNG skipped, tidak diimpor;
    /// (4) TIDAK ADA password ter-import (credential_id sengaja tanpa
    /// secret — baca ulang harus gagal); (5) import file yang SAMA dua
    /// kali TIDAK menduplikasi grup (dedup by name).
    #[test]
    fn import_securecrt_flatten_group_path_dan_dedup_grup() {
        let mut vault = temp_vault();
        vault.initialize("master-password-test").unwrap();

        let parsed = terminus_core::import::securecrt::parse(IMPORT_SAMPLE_XML).unwrap();
        assert_eq!(parsed.hosts.len(), 3);
        assert_eq!(parsed.skipped, 1, "sesi Serial harus kehitung skipped");

        let summary = import_parsed_hosts_into_vault(&mut vault, parsed).unwrap();
        assert_eq!(summary.imported, 3);
        assert_eq!(summary.skipped, 1);

        let groups = vault.list_groups().unwrap();
        assert_eq!(groups.len(), 1, "rtr-01 & rtr-02 di folder yang sama harus dapat SATU grup, bukan dua");
        assert_eq!(groups[0].name, "ROUTER / BAROKAH");

        let profiles = vault.list_all_profiles().unwrap();
        assert_eq!(profiles.len(), 3);
        let rtr01 = profiles.iter().find(|p| p.label == "rtr-01").unwrap();
        let rtr02 = profiles.iter().find(|p| p.label == "rtr-02").unwrap();
        let standalone = profiles.iter().find(|p| p.label == "standalone-host").unwrap();
        assert_eq!(rtr01.group_id, Some(groups[0].id));
        assert_eq!(rtr02.group_id, Some(groups[0].id), "rtr-01 & rtr-02 harus di grup YANG SAMA");
        assert_eq!(standalone.group_id, None, "host di root Sessions harus ungrouped");
        assert_eq!(standalone.host, "10.2.2.2");
        assert_eq!(standalone.port, 22);
        assert!(standalone.tags.contains(&"imported".to_string()));

        // TIDAK ADA password yang ke-import — baca secret manapun harus gagal.
        for p in &profiles {
            let AuthMethod::Password { credential_id } = p.auth else { panic!("harus AuthMethod::Password") };
            assert!(vault.read_secret(credential_id).is_err(), "host hasil import TIDAK BOLEH punya secret tersimpan");
        }

        // Import ULANG file yang sama -> grup TIDAK boleh terduplikasi
        // (harus reuse "ROUTER / BAROKAH" yang sudah ada), meski host
        // baru (id beda) tetap kebuat lagi (itu ekspektasi wajar untuk
        // import naif tanpa dedup-by-hostname — di luar scope v1 ini).
        let parsed_again = terminus_core::import::securecrt::parse(IMPORT_SAMPLE_XML).unwrap();
        import_parsed_hosts_into_vault(&mut vault, parsed_again).unwrap();
        let groups_after = vault.list_groups().unwrap();
        assert_eq!(groups_after.len(), 1, "import ulang TIDAK BOLEH bikin grup 'ROUTER / BAROKAH' dobel");
    }

    /// Poll `check` tiap beberapa milidetik sampai `true` atau timeout
    /// (panik) — dipakai buat nunggu efek satu `invoke_*` yang sekarang
    /// async beneran nyampe ke UI thread lewat `invoke_from_event_loop`.
    async fn wait_until(mut check: impl FnMut() -> bool, what: &str) {
        for _ in 0..500 {
            if check() {
                return;
            }
            tokio::time::sleep(StdDuration::from_millis(10)).await;
        }
        panic!("timeout nunggu: {what}");
    }

    #[test]
    fn alur_hosts_lengkap_create_group_invarian_drilldown_delete() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _guard = rt.enter();

        let ui = AppWindow::new().unwrap();
        let state = wire_callbacks(&ui, temp_vault());
        let ui_weak = ui.as_weak();

        slint::spawn_local(async move {
            let ui = ui_weak.upgrade().unwrap();

            // First run -> buat master password sekaligus unlock.
            assert!(ui.global::<VaultModel>().get_is_first_run());
            ui.global::<VaultModel>().invoke_create_requested("master-password-test".into());
            wait_until(|| ui.global::<VaultModel>().get_is_unlocked(), "vault unlock setelah create").await;

            // Host pertama, tanpa grup, dengan tags.
            ui.global::<HostsModel>().invoke_create_host_requested(NewHostForm {
                label: "host-a".into(),
                host: "10.0.0.1".into(),
                port: 22,
                username: "root".into(),
                password: "pw123".into(),
                kind: "ssh".into(),
                group_id: "".into(),
                tags: " production ,  , database ".into(), // sengaja berantakan
            });
            wait_until(|| ui.global::<HostsModel>().get_ungrouped_hosts().row_count() == 1, "host-a muncul di List")
                .await;
            let ungrouped = ui.global::<HostsModel>().get_ungrouped_hosts();
            assert_eq!(ungrouped.row_data(0).unwrap().label, "host-a");
            assert_eq!(
                ungrouped.row_data(0).unwrap().tags,
                "production, database",
                "tags harus di-trim & entry kosong dibuang"
            );

            // Bikin grup.
            ui.global::<HostsModel>().invoke_create_group_requested("Production".into(), "us-east-1".into());
            wait_until(|| ui.global::<HostsModel>().get_groups().row_count() == 1, "grup Production tersimpan").await;
            let groups = ui.global::<HostsModel>().get_groups();
            let group = groups.row_data(0).unwrap();
            assert_eq!(group.name, "Production");
            assert_eq!(group.total_count, 0, "grup baru belum ada host di dalamnya");
            let group_id = group.id;

            // Host kedua, LANGSUNG dimasukkan ke grup itu waktu dibuat.
            ui.global::<HostsModel>().invoke_create_host_requested(NewHostForm {
                label: "host-b".into(),
                host: "10.0.0.2".into(),
                port: 22,
                username: "root".into(),
                password: "pw456".into(),
                kind: "ssh".into(),
                group_id: group_id.clone(),
                tags: "".into(),
            });
            wait_until(
                || ui.global::<HostsModel>().get_groups().row_data(0).map(|g| g.total_count) == Some(1),
                "host-b kehitung di grup Production",
            )
            .await;

            // INVARIAN INTI: host-b masuk grup -> TIDAK boleh nongol lagi
            // di daftar List (ungrouped). host-a (ungrouped) tetap ada.
            let ungrouped = ui.global::<HostsModel>().get_ungrouped_hosts();
            assert_eq!(ungrouped.row_count(), 1, "host-b tidak boleh ada di List setelah masuk grup");
            assert_eq!(ungrouped.row_data(0).unwrap().label, "host-a");

            // Drill-down: buka grup, host-b harus muncul di sana.
            ui.global::<HostsModel>().invoke_group_opened(group_id.clone());
            assert_eq!(ui.global::<HostsModel>().get_selected_group_name(), "Production");
            let active = ui.global::<HostsModel>().get_active_group_hosts();
            assert_eq!(active.row_count(), 1);
            assert_eq!(active.row_data(0).unwrap().label, "host-b");

            // Hapus grup -> host-b (di dalamnya) HARUS ikut kehapus
            // permanen (CASCADE, lihat VaultStore::delete_group — perubahan
            // perilaku SENGAJA atas permintaan eksplisit user, sebelumnya
            // host-b cuma balik jadi ungrouped). host-a (di luar grup)
            // TIDAK boleh ikut kena.
            ui.global::<HostsModel>().invoke_group_delete_requested(group_id);
            wait_until(|| ui.global::<HostsModel>().get_groups().row_count() == 0, "grup Production terhapus").await;
            wait_until(|| !ui.global::<HostsModel>().get_confirm_delete_group_open(), "dialog konfirmasi grup tertutup").await;
            let ungrouped_after = ui.global::<HostsModel>().get_ungrouped_hosts();
            assert_eq!(ungrouped_after.row_count(), 1, "host-b ikut terhapus (cascade), host-a TIDAK boleh ikut kena");
            assert_eq!(ungrouped_after.row_data(0).unwrap().label, "host-a");

            // host-b (versi lama) sudah permanen hilang lewat cascade di
            // atas — bagian test SELANJUTNYA (pencarian host ungrouped +
            // hapus satuan) butuh SATU host ungrouped independen buat
            // dites, jadi dibuat ulang di sini (label sama, id BEDA —
            // bukan "kebangkitan" host lama, murni host baru).
            ui.global::<HostsModel>().invoke_create_host_requested(NewHostForm {
                label: "host-b".into(),
                host: "10.0.0.20".into(),
                port: 22,
                username: "root".into(),
                password: "pw789".into(),
                kind: "ssh".into(),
                group_id: "".into(),
                tags: "".into(),
            });
            wait_until(
                || ui.global::<HostsModel>().get_ungrouped_hosts().row_count() == 2,
                "host-b (baru, ungrouped) muncul lagi di List",
            )
            .await;

            // --- Edit host: rename host-a + pindahkan ke grup baru ---
            // Cari berdasarkan label, JANGAN asumsikan urutan baris SQL
            // (SELECT tanpa ORDER BY tidak menjamin urutan insert).
            let host_a_id = (0..ungrouped_after.row_count())
                .map(|i| ungrouped_after.row_data(i).unwrap())
                .find(|h| h.label == "host-a")
                .expect("host-a harus ada di ungrouped setelah grup Production dihapus")
                .id;

            // Klik kartu host-a -> panel "Host Details" harus keisi data
            // asli host-a (tanpa password, itu tetap rahasia; tags harus
            // sudah ke-normalize dari waktu create tadi). Ini SINKRON
            // (baca doang, bukan mutasi), tidak butuh wait_until.
            ui.global::<HostsModel>().invoke_host_selected(host_a_id.clone());
            assert!(ui.global::<HostsModel>().get_panel_visible());
            assert_eq!(ui.global::<HostsModel>().get_panel_host_label(), "host-a");
            assert_eq!(ui.global::<HostsModel>().get_panel_host_host(), "10.0.0.1");
            assert_eq!(ui.global::<HostsModel>().get_panel_host_port(), 22);
            assert_eq!(ui.global::<HostsModel>().get_panel_host_group_id(), "", "host-a belum masuk grup manapun");
            assert_eq!(ui.global::<HostsModel>().get_panel_host_tags(), "production, database");

            ui.global::<HostsModel>().invoke_create_group_requested("Staging".into(), "".into());
            wait_until(|| ui.global::<HostsModel>().get_groups().row_count() == 1, "grup Staging tersimpan").await;
            let staging_id = ui.global::<HostsModel>().get_groups().row_data(0).unwrap().id;

            // Tombol Simpan: ganti label + host + pindahkan ke grup
            // Staging, password DIKOSONGKAN (harus tetap bisa connect
            // pakai password lama).
            ui.global::<HostsModel>().invoke_host_save_requested(
                host_a_id.clone(),
                NewHostForm {
                    label: "host-a-renamed".into(),
                    host: "10.0.0.99".into(),
                    port: 2222,
                    username: "root".into(),
                    password: "".into(),
                    kind: "ssh".into(),
                    group_id: staging_id.clone(),
                    tags: "web".into(),
                },
            );
            wait_until(
                || ui.global::<HostsModel>().get_panel_host_label() == "host-a-renamed",
                "panel ter-refresh setelah Simpan",
            )
            .await;

            // host-a-renamed harus HILANG dari List (sekarang di grup)...
            let ungrouped_final = ui.global::<HostsModel>().get_ungrouped_hosts();
            assert!(
                !ungrouped_final.iter().any(|h| h.id == host_a_id),
                "host-a harus hilang dari List setelah dipindah ke grup"
            );
            assert_eq!(ui.global::<HostsModel>().get_panel_host_tags(), "web");
            // ...dan muncul di grup Staging dengan data yang sudah diedit.
            ui.global::<HostsModel>().invoke_group_opened(staging_id.clone());
            let staging_hosts = ui.global::<HostsModel>().get_active_group_hosts();
            assert_eq!(staging_hosts.row_count(), 1);
            let edited = staging_hosts.row_data(0).unwrap();
            assert_eq!(edited.label, "host-a-renamed");
            assert_eq!(edited.address, "10.0.0.99:2222");

            // --- Duplicate: host-a-renamed digandakan, credential_id
            // HARUS beda dari aslinya (deep copy, bukan berbagi referensi)
            // — dicek langsung ke vault, bukan lewat Slint. ---
            let original_credential_id = match state
                .vault
                .lock()
                .unwrap()
                .list_all_profiles()
                .unwrap()
                .into_iter()
                .find(|p| p.id.to_string() == host_a_id.as_str())
                .unwrap()
                .auth
            {
                AuthMethod::Password { credential_id } => credential_id,
                _ => panic!("host-a-renamed harus pakai auth Password"),
            };

            ui.global::<HostsModel>().invoke_host_duplicate_requested(host_a_id.clone());
            wait_until(
                || ui.global::<HostsModel>().get_panel_host_label() == "host-a-renamed (copy)",
                "duplikat tersimpan & panel pindah ke situ",
            )
            .await;
            let duplicate_id = ui.global::<HostsModel>().get_panel_host_id();
            assert_ne!(duplicate_id, host_a_id, "duplikat harus punya id baru");

            // Duplikat ikut masuk grup Staging yang sama (tercopy dari asli).
            ui.global::<HostsModel>().invoke_group_opened(staging_id);
            assert_eq!(
                ui.global::<HostsModel>().get_active_group_hosts().row_count(),
                2,
                "grup Staging sekarang harus berisi host-a-renamed + duplikatnya"
            );

            let duplicate_credential_id = match state
                .vault
                .lock()
                .unwrap()
                .list_all_profiles()
                .unwrap()
                .into_iter()
                .find(|p| p.id.to_string() == duplicate_id.as_str())
                .unwrap()
                .auth
            {
                AuthMethod::Password { credential_id } => credential_id,
                _ => panic!("duplikat harus pakai auth Password"),
            };
            assert_ne!(
                original_credential_id, duplicate_credential_id,
                "duplicate harus deep-copy secret ke credential_id baru, bukan berbagi referensi"
            );

            // Hapus duplikatnya (langsung panggil `host_delete_requested`,
            // persis yang dilakukan dialog konfirmasi Slint setelah user
            // klik "Hapus") -> panel yang lagi nampilin dia harus ikut
            // tertutup otomatis.
            ui.global::<HostsModel>().invoke_host_delete_requested(duplicate_id);
            wait_until(|| !ui.global::<HostsModel>().get_panel_visible(), "panel tertutup setelah duplikat dihapus")
                .await;

            // --- Pencarian: HARUS ketemu host di dalam grup (bukan
            // cuma yang di List) — sinkron, tidak butuh wait_until.
            // Di titik ini: host-a-renamed ada di grup Staging,
            // host-b masih ungrouped di List.
            ui.global::<HostsModel>().invoke_search_requested("host-a-renamed".into());
            let grouped_hit = ui.global::<HostsModel>().get_search_results();
            assert_eq!(grouped_hit.row_count(), 1, "pencarian harus ketemu host di DALAM grup, bukan cuma di List");
            assert_eq!(grouped_hit.row_data(0).unwrap().label, "host-a-renamed");

            ui.global::<HostsModel>().invoke_search_requested("host-b".into());
            let ungrouped_hit = ui.global::<HostsModel>().get_search_results();
            assert_eq!(ungrouped_hit.row_count(), 1, "pencarian harus tetap ketemu host ungrouped di List");
            assert_eq!(ungrouped_hit.row_data(0).unwrap().label, "host-b");

            // Query yang tidak match siapa-siapa -> hasil kosong,
            // bukan error/panik.
            ui.global::<HostsModel>().invoke_search_requested("host-yang-tidak-ada".into());
            assert_eq!(ui.global::<HostsModel>().get_search_results().row_count(), 0);

            // Query kosong (mis. user klik "×" clear) -> balik kosong juga.
            ui.global::<HostsModel>().invoke_search_requested("".into());
            assert_eq!(ui.global::<HostsModel>().get_search_results().row_count(), 0);

            // Hapus satu host — pakai snapshot List TERBARU (bukan
            // `ungrouped_after` yang sudah basi sejak host-a dipindah ke
            // grup), harusnya cuma tersisa host-b di sana sekarang.
            assert_eq!(ungrouped_final.row_count(), 1, "cuma host-b yang harus tersisa di List");
            let victim_id = ungrouped_final.row_data(0).unwrap().id;
            ui.global::<HostsModel>().invoke_host_delete_requested(victim_id);
            wait_until(
                || ui.global::<HostsModel>().get_ungrouped_hosts().row_count() == 0,
                "host-b terhapus, List jadi kosong",
            )
            .await;

            // --- Slide-over mode "New Host": tombol Connect simpan
            // DULU baru langsung connect (`host-connect-new-requested`)
            // — beda dari `create-host-requested` (Simpan biasa). Pakai
            // alamat yang PASTI gagal connect (port 1, jarang ada yang
            // listen di situ tanpa root) biar test cepat & tidak butuh
            // server sungguhan — yang mau dibuktikan cuma "host-nya
            // BENERAN tersimpan ke vault DAN percobaan connect BENERAN
            // jalan", bukan hasil connect-nya. ---
            ui.global::<HostsModel>().set_notice_message("".into());
            ui.global::<HostsModel>().invoke_host_connect_new_requested(NewHostForm {
                label: "quick-connect-host".into(),
                host: "127.0.0.1".into(),
                port: 1,
                username: "nobody".into(),
                password: "irrelevant".into(),
                kind: "ssh".into(),
                group_id: "".into(),
                tags: "".into(),
            });
            wait_until(
                || ui.global::<HostsModel>().get_ungrouped_hosts().iter().any(|h| h.label == "quick-connect-host"),
                "host dari Connect (mode New Host) beneran tersimpan ke vault",
            )
            .await;
            wait_until(
                || !ui.global::<HostsModel>().get_notice_message().is_empty(),
                "percobaan connect beneran jalan (gagal ke 127.0.0.1:1, notice muncul)",
            )
            .await;
            assert!(ui.global::<HostsModel>().get_notice_is_error(), "connect ke port yang tidak ada harus gagal");

            // --- Bagian 2: unlock dengan password salah ditolak ---
            // Digabung ke test yang sama (bukan #[test] terpisah) karena
            // winit cuma bisa init event loop SEKALI per proses — dua
            // `AppWindow::new()` di dua #[test] yang jalan paralel bikin
            // "EventLoop can't be recreated". `ui` yang sama dipakai ulang.
            ui.global::<VaultModel>().set_is_unlocked(false);
            ui.global::<VaultModel>().invoke_unlock_requested("password-salah".into());
            wait_until(
                || !ui.global::<VaultModel>().get_error_message().is_empty(),
                "pesan error muncul setelah password salah",
            )
            .await;
            assert!(!ui.global::<VaultModel>().get_is_unlocked(), "tidak boleh unlock dengan password salah");

            // Password benar tetap bisa unlock setelahnya.
            ui.global::<VaultModel>().invoke_unlock_requested("master-password-test".into());
            wait_until(|| ui.global::<VaultModel>().get_is_unlocked(), "unlock dengan password yang benar").await;

            slint::quit_event_loop().unwrap();
        })
        .unwrap();

        slint::run_event_loop_until_quit().unwrap();
    }
}
