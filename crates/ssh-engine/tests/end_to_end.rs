//! Test integrasi end-to-end: `terminus-vault` (encrypted secret
//! storage) + koneksi SSH BENERAN ke server lokal — bukan mock.
//!
//! Butuh server SSH aktif di `127.0.0.1:2222` dengan user `root` /
//! password `testpass123`. Cara paling gampang jalanin server test-nya
//! (lihat README bagian "Testing ssh-engine"):
//!
//! ```bash
//! docker run -d --name terminus-test-sshd -p 2222:22 alpine:3.20 sh -c "
//!   apk add --no-cache openssh >/dev/null 2>&1
//!   echo 'root:testpass123' | chpasswd
//!   sed -i 's/#PermitRootLogin.*/PermitRootLogin yes/' /etc/ssh/sshd_config
//!   sed -i 's/#PasswordAuthentication.*/PasswordAuthentication yes/' /etc/ssh/sshd_config
//!   ssh-keygen -A >/dev/null 2>&1
//!   /usr/sbin/sshd -D
//! "
//! cargo test -p terminus-ssh-engine --test end_to_end -- --ignored
//! ```
//!
//! Di-`#[ignore]` supaya `cargo test` biasa (tanpa server itu) tetap
//! hijau di mesin siapapun.

use terminus_core::{AuthMethod, ConnectionKind, HostProfile};
use terminus_ssh_engine::{connect, HostKeyStore, SecretMaterial, SshEngineError, SshOutputEvent};
use terminus_vault::VaultStore;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use uuid::Uuid;

const TEST_HOST: &str = "127.0.0.1";
const TEST_PORT: u16 = 2222;
const TEST_USER: &str = "root";
const TEST_PASSWORD: &str = "testpass123";

/// Implementasi `HostKeyStore` minimal buat test — persis pola yang
/// dipakai `crates/app/src/host_key_store.rs` di aplikasi beneran,
/// cuma dituliskan ulang di sini karena `terminus-app` adalah binary
/// (tidak expose lib buat dipakai test crate lain).
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
    let path = std::env::temp_dir().join(format!("terminus-e2e-{}.db", Uuid::new_v4()));
    let mut store = VaultStore::open_at(path).unwrap();
    store.initialize("master-password-test").unwrap();
    store
}

fn test_profile(credential_id: Uuid) -> HostProfile {
    HostProfile {
        id: Uuid::new_v4(),
        label: "test-alpine-sshd".into(),
        host: TEST_HOST.into(),
        port: TEST_PORT,
        username: TEST_USER.into(),
        kind: ConnectionKind::Ssh,
        auth: AuthMethod::Password { credential_id },
        group_id: None,
        tags: vec![],
        terminal_theme: None,
    }
}

#[tokio::test]
#[ignore = "butuh server SSH lokal di 127.0.0.1:2222 (root/testpass123) — lihat komentar di atas"]
async fn connect_beneran_baca_password_terenkripsi_dan_tukar_data() {
    let vault = Arc::new(Mutex::new(temp_vault()));
    let credential_id = Uuid::new_v4();
    vault.lock().unwrap().store_secret(credential_id, TEST_PASSWORD.as_bytes()).unwrap();

    // Password dibaca lewat vault (didecrypt di sini), PERSIS alur
    // yang dipakai `crates/app/src/state.rs::on_host_connect_requested`.
    let password_bytes = vault.lock().unwrap().read_secret(credential_id).unwrap();
    let password = String::from_utf8(password_bytes).unwrap();
    assert_eq!(password, TEST_PASSWORD, "roundtrip encrypt/decrypt vault harus balik ke plaintext asli");

    let profile = test_profile(credential_id);
    let host_key_store: Arc<dyn HostKeyStore> = Arc::new(TestHostKeyStore { vault: vault.clone() });

    let mut session = connect(&profile, SecretMaterial::Password(password), host_key_store)
        .await
        .expect("koneksi SSH ke server test harus sukses (cek server test jalan di 127.0.0.1:2222)");

    // Host key harus otomatis ke-TOFU-simpan setelah connect sukses.
    let stored = vault.lock().unwrap().check_host_key(TEST_HOST, TEST_PORT).unwrap();
    assert!(stored.is_some(), "host key harus otomatis tersimpan (trust-on-first-use)");

    // Buktikan channel beneran dua arah: kirim command, terima output.
    let mut rx = session.subscribe_output();
    session.write(b"echo terminus_E2E_OK\n").await.unwrap();

    let mut got_expected_output = false;
    for _ in 0..20 {
        if let Ok(Ok(SshOutputEvent::Data(data))) = tokio::time::timeout(Duration::from_millis(500), rx.recv()).await
        {
            if String::from_utf8_lossy(&data).contains("terminus_E2E_OK") {
                got_expected_output = true;
                break;
            }
        }
    }
    assert!(got_expected_output, "harus nerima output echo dari server sungguhan");

    session.disconnect().await.unwrap();

    // Setelah disconnect, subscriber HARUS nerima sinyal `Closed` —
    // ini yang dipakai `crates/app` buat otomatis balikin status host
    // ke "offline" tanpa perlu polling.
    let mut got_closed = false;
    for _ in 0..20 {
        match tokio::time::timeout(Duration::from_millis(500), rx.recv()).await {
            Ok(Ok(SshOutputEvent::Closed)) => {
                got_closed = true;
                break;
            }
            Ok(Ok(SshOutputEvent::Data(_))) => continue,
            _ => break,
        }
    }
    assert!(got_closed, "subscriber harus dapat sinyal Closed setelah sesi diputus");
}

#[tokio::test]
#[ignore = "butuh server SSH lokal di 127.0.0.1:2222 — lihat komentar di atas"]
async fn host_key_yang_berubah_dari_tersimpan_ditolak() {
    let vault = Arc::new(Mutex::new(temp_vault()));
    // Simpan host key PALSU (bukan milik server test beneran) di muka
    // — mensimulasikan "server ganti identitas" / potensi MITM.
    vault
        .lock()
        .unwrap()
        .trust_host_key(
            TEST_HOST,
            TEST_PORT,
            "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIFAKEuntukTESTmismatchBUKANkunciASLI",
        )
        .unwrap();

    let credential_id = Uuid::new_v4();
    vault.lock().unwrap().store_secret(credential_id, TEST_PASSWORD.as_bytes()).unwrap();
    let profile = test_profile(credential_id);
    let host_key_store: Arc<dyn HostKeyStore> = Arc::new(TestHostKeyStore { vault: vault.clone() });

    let result = connect(&profile, SecretMaterial::Password(TEST_PASSWORD.into()), host_key_store).await;

    match result {
        Err(SshEngineError::HostKeyMismatch(_)) => {}
        Err(other) => panic!("harus ditolak sebagai HostKeyMismatch, malah dapat error lain: {other}"),
        Ok(_) => panic!("koneksi HARUS ditolak waktu host key beda dari yang tersimpan, malah sukses"),
    }
}
