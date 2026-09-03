//! Test integrasi end-to-end buat fitur terminal interaktif: SSH
//! BENERAN (bukan mock) -> `TerminalInstance` (parser VTE) -> grid
//! karakter yang bisa dibaca teksnya. Ini nutup satu-satunya link yang
//! belum kebukti sama test lain: `crates/ssh-engine/tests/end_to_end.rs`
//! cuma buktiin byte mentah sampai; `crates/term-emulator/src/lib.rs`
//! cuma dites pakai string sintetis (`term.feed(b"...")`). Di sini
//! keduanya DIGABUNG dengan server SSH sungguhan, persis alur yang
//! dijalankan `crates/app/src/state.rs::spawn_terminal_reader` (minus
//! plumbing Slint-nya, yang butuh event loop beneran jalan — lihat
//! komentar di `state.rs` test module).
//!
//! Butuh server SSH aktif di `127.0.0.1:2222` — sama seperti
//! `terminus-ssh-engine`'s end_to_end.rs, lihat komentar di file itu
//! buat perintah `docker run`-nya.
//!
//! ```bash
//! cargo test -p terminus-app --test ssh_terminal_e2e -- --ignored
//! ```

use terminus_core::{AuthMethod, ConnectionKind, HostProfile};
use terminus_ssh_engine::{connect, HostKeyStore, SecretMaterial, SshOutputEvent};
use terminus_term_emulator::TerminalInstance;
use terminus_vault::VaultStore;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use uuid::Uuid;

const TEST_HOST: &str = "127.0.0.1";
const TEST_PORT: u16 = 2222;
const TEST_USER: &str = "root";
const TEST_PASSWORD: &str = "testpass123";

struct TestHostKeyStore {
    vault: Arc<Mutex<VaultStore>>,
}

impl HostKeyStore for TestHostKeyStore {
    fn lookup(&self, host: &str, port: u16) -> Option<String> {
        self.vault.lock().ok()?.check_host_key(host, port).ok().flatten()
    }
    fn trust(&self, host: &str, port: u16, key_line: String) {
        let _ = self.vault.lock().unwrap().trust_host_key(host, port, &key_line);
    }
}

fn temp_vault() -> VaultStore {
    let path = std::env::temp_dir().join(format!("terminus-term-e2e-{}.db", Uuid::new_v4()));
    let mut store = VaultStore::open_at(path).unwrap();
    store.initialize("master-password-test").unwrap();
    store
}

#[tokio::test]
#[ignore = "butuh server SSH lokal di 127.0.0.1:2222 (root/testpass123) — lihat komentar di atas"]
async fn perintah_beneran_lewat_ssh_muncul_di_grid_terminal() {
    let vault = Arc::new(Mutex::new(temp_vault()));
    let credential_id = Uuid::new_v4();
    vault.lock().unwrap().store_secret(credential_id, TEST_PASSWORD.as_bytes()).unwrap();

    let profile = HostProfile {
        id: Uuid::new_v4(),
        label: "test-alpine-sshd".into(),
        host: TEST_HOST.into(),
        port: TEST_PORT,
        username: TEST_USER.into(),
        kind: ConnectionKind::Ssh,
        auth: AuthMethod::Password { credential_id },
        group_id: None,
        tags: vec![],
    };
    let host_key_store: Arc<dyn HostKeyStore> = Arc::new(TestHostKeyStore { vault: vault.clone() });

    // --- Persis alur `state.rs::on_host_connect_requested` ---
    let mut session = connect(&profile, SecretMaterial::Password(TEST_PASSWORD.into()), host_key_store)
        .await
        .expect("koneksi SSH ke server test harus sukses (cek server test jalan di 127.0.0.1:2222)");
    session.resize_pty(80, 24).await.unwrap();
    let mut rx = session.subscribe_output();

    // --- Persis alur `state.rs::spawn_terminal_reader` ---
    let mut term = TerminalInstance::new(80, 24);

    // Kirim command sekali shell siap. Server pakai `ash` (Alpine) yang
    // langsung ngasih prompt begitu channel shell dibuka — tunggu
    // sebentar biar prompt awal kekirim duluan, baru command kita.
    tokio::time::sleep(Duration::from_millis(500)).await;
    session.write(b"echo terminus_TERM_UI_OK\r").await.unwrap();

    // Feed semua byte yang datang ke parser VTE beneran (BUKAN string
    // sintetis) sampai grid-nya kelihatan mengandung output command.
    let mut found = false;
    for _ in 0..30 {
        match tokio::time::timeout(Duration::from_millis(500), rx.recv()).await {
            Ok(Ok(SshOutputEvent::Data(bytes))) => {
                term.feed(&bytes);
                let snap = term.snapshot(&terminus_term_emulator::palette::terminus_dark());
                let screen_text: String = snap.cells.iter().map(|c| c.ch).collect();
                if screen_text.contains("terminus_TERM_UI_OK") {
                    found = true;
                    break;
                }
            }
            Ok(Ok(SshOutputEvent::Closed)) => break,
            _ => continue,
        }
    }
    assert!(found, "output command asli dari server harus ke-render di grid terminal (bukan cuma diterima mentah)");

    session.disconnect().await.unwrap();
}
