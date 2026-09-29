//! E2E `RemoteVaultClient` lawan backend SUNGGUHAN (`backend/`, `npm run
//! dev`) — Milestone 6 `docs/desktop-selfhosted-integration.md`. Urutan
//! panggilan SENGAJA meniru `crates/app/src/state.rs` apa adanya
//! (termasuk `credential_id` host/identity yang BEDA dari `id`-nya dan
//! `store_secret` SEBELUM `save_profile`/`save_identity`), karena justru
//! kontrak itulah yang diuji bagian 2.6 doc.
//!
//! `#[ignore]` — butuh server jalan. JANGAN arahkan ke database yang
//! berisi akun sungguhan (registrasi publik tertutup begitu ada user,
//! dan test ini meninggalkan akun uji). Pakai instance TERPISAH dengan
//! database sendiri, mis. `terminus_e2e`:
//! `cd backend && PORT=4100 DATABASE_URL=mysql://…/terminus_e2e npx tsx
//! src/index.ts` (sekali: `DATABASE_URL=… npx prisma migrate deploy`),
//! lalu `TERMINUS_E2E_URL=http://localhost:4100 cargo test -p
//! terminus-vault --test remote_e2e -- --ignored --nocapture`
//!
//! Akun uji: `TERMINUS_E2E_EMAIL`/`TERMINUS_E2E_PASSWORD` (default
//! `e2e@terminus.local`/`e2e-password-123`) — dicoba register dulu
//! (sukses kalau server masih kosong), kalau ditolak baru login.

use terminus_core::{AuthMethod, ConnectionKind, HostGroup, HostProfile, Identity};
use terminus_vault::{RemoteVaultClient, VaultError};
use uuid::Uuid;

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

async fn login_or_register(url: &str) -> terminus_vault::AuthTokens {
    let email = env_or("TERMINUS_E2E_EMAIL", "e2e@terminus.local");
    let password = env_or("TERMINUS_E2E_PASSWORD", "e2e-password-123");
    match RemoteVaultClient::register(url, &email, &password, "Akun E2E").await {
        Ok(tokens) => tokens,
        Err(_) => RemoteVaultClient::login(url, &email, &password).await.expect("login akun uji"),
    }
}

fn new_host(group_id: Option<Uuid>) -> (HostProfile, Uuid) {
    // Persis `on_host_save_requested` (state.rs): id & credential_id
    // DUA UUID berbeda.
    let credential_id = Uuid::new_v4();
    let profile = HostProfile {
        id: Uuid::new_v4(),
        label: format!("e2e-host-{}", &Uuid::new_v4().to_string()[..8]),
        host: "10.9.8.7".into(),
        port: 22,
        username: "admin".into(),
        kind: ConnectionKind::Ssh,
        auth: AuthMethod::Password { credential_id },
        group_id,
        tags: vec![],
        terminal_theme: None,
    };
    (profile, credential_id)
}

#[tokio::test]
#[ignore]
async fn alur_lengkap_lawan_backend_sungguhan() {
    let url = env_or("TERMINUS_E2E_URL", "http://localhost:4000");

    // --- Auth + resolusi vault (auth_flow::resolve_vault_id) ---
    let tokens = login_or_register(&url).await;
    let vaults = RemoteVaultClient::list_vaults(&url, &tokens.access_token).await.expect("list vault");
    let vault_id = match vaults.into_iter().next() {
        Some(v) => v.id,
        None => RemoteVaultClient::create_vault(&url, &tokens.access_token, "My Vault").await.expect("create vault").id,
    };
    println!("vault = {vault_id}");

    // Access token SENGAJA rusak -> request pertama 401 -> client wajib
    // refresh diam-diam pakai refresh token lalu ulangi (bagian 2.5).
    let rotated = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let rotated_sink = rotated.clone();
    let client = RemoteVaultClient::new(url.clone(), vault_id, "token-rusak".into(), tokens.refresh_token.clone())
        .with_token_persister(move |t| rotated_sink.lock().unwrap().push(t));

    // --- Grup ---
    let group = HostGroup { id: Uuid::new_v4(), name: "e2e-grup".into(), subtitle: None, parent_id: None };
    client.save_group(&group).await.expect("create grup (lewat refresh-on-401)");
    assert_eq!(rotated.lock().unwrap().len(), 1, "refresh token hasil rotasi harus dipersist");
    assert_ne!(client.current_refresh_token().await, tokens.refresh_token, "refresh token harus sudah rotate");
    assert!(client.list_groups().await.unwrap().iter().any(|g| g.id == group.id), "id grup dari client dipakai apa adanya");

    // --- Host BARU + password (store_secret SEBELUM save_profile) ---
    let (host, credential_id) = new_host(Some(group.id));
    client.store_secret(credential_id, b"rahasia-host-1").await.expect("store_secret host baru (harus dibuffer)");
    client.save_profile(&host).await.expect("save_profile host baru + flush password");

    let listed = client.list_all_profiles().await.unwrap();
    let from_server = listed.iter().find(|p| p.id == host.id).expect("host baru ada di server dengan id dari client");
    let server_cred = match from_server.auth {
        AuthMethod::Password { credential_id } => credential_id,
        _ => panic!("auth harus Password"),
    };
    assert_eq!(server_cred, host.id, "credential_id dari server == id host (bagian 2.6)");
    assert_eq!(client.read_secret(server_cred).await.unwrap(), b"rahasia-host-1");

    // --- Host hasil import SecureCRT: TANPA username & TANPA password
    // (persis `import_parsed_hosts_into_vault` di state.rs) ---
    let (mut imported, _) = new_host(None);
    imported.username = String::new();
    imported.tags = vec!["imported".into()];
    client.save_profile(&imported).await.expect("host tanpa username harus bisa disimpan (import config.xml)");
    let back = client.list_all_profiles().await.unwrap().into_iter().find(|p| p.id == imported.id).expect("host import ada");
    assert_eq!(back.username, "");
    client.delete_profile(imported.id).await.unwrap();

    // --- UPDATE host yang sudah ada: ganti label + password ---
    let mut edited = from_server.clone();
    edited.label = format!("{}-edit", edited.label);
    client.store_secret(server_cred, b"rahasia-host-2").await.expect("store_secret host lama (PUT langsung)");
    client.save_profile(&edited).await.expect("update host");
    assert_eq!(client.read_secret(server_cred).await.unwrap(), b"rahasia-host-2");
    assert!(client.list_all_profiles().await.unwrap().iter().any(|p| p.id == host.id && p.label == edited.label));

    // --- Identity BARU (password wajib ikut POST) ---
    let identity = Identity {
        id: Uuid::new_v4(),
        label: "e2e-identity".into(),
        username: "noc".into(),
        credential_id: Uuid::new_v4(),
    };
    client.store_secret(identity.credential_id, b"rahasia-identity").await.expect("store_secret identity baru");
    client.save_identity(&identity).await.expect("save_identity baru");
    let identities = client.list_identities().await.unwrap();
    let id_from_server = identities.iter().find(|i| i.id == identity.id).expect("identity ada di server");
    assert_eq!(client.read_secret(id_from_server.credential_id).await.unwrap(), b"rahasia-identity");

    // --- Hapus: identity, host, grup (cascade) ---
    client.delete_identity(identity.id).await.expect("hapus identity");
    let (host2, cred2) = new_host(Some(group.id));
    client.store_secret(cred2, b"x").await.unwrap();
    client.save_profile(&host2).await.unwrap();
    client.delete_group(group.id).await.expect("hapus grup");
    let after = client.list_all_profiles().await.unwrap();
    assert!(!after.iter().any(|p| p.id == host.id || p.id == host2.id), "hapus grup ikut hapus host di dalamnya");
    assert!(!client.list_identities().await.unwrap().iter().any(|i| i.id == identity.id));

    // --- Login diam-diam: refresh token LAMA sudah dicabut (rotasi),
    // yang TERBARU masih valid ---
    match RemoteVaultClient::refresh_session(&url, &tokens.refresh_token).await {
        Err(VaultError::SessionExpired(_)) => {}
        other => panic!("refresh token lama harus SessionExpired, dapat: {:?}", other.map(|_| ())),
    }
    let latest = client.current_refresh_token().await;
    let fresh = RemoteVaultClient::refresh_session(&url, &latest).await.expect("refresh token terbaru harus valid");

    // --- Email akun (sidebar) + logout mencabut token TERBARU ---
    let email = RemoteVaultClient::me(&url, &fresh.access_token).await.expect("GET /auth/me").email;
    assert_eq!(email, env_or("TERMINUS_E2E_EMAIL", "e2e@terminus.local"));
    let session = RemoteVaultClient::new(url.clone(), "tidak-dipakai".into(), fresh.access_token, fresh.refresh_token.clone());
    session.logout().await.expect("logout");

    // Race logout vs refresh di background: setelah ditandai logout,
    // token hasil rotasi TIDAK BOLEH dipersist lagi.
    let after_logout = RemoteVaultClient::login(&url, &email, &env_or("TERMINUS_E2E_PASSWORD", "e2e-password-123")).await.unwrap();
    let persisted = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let sink = persisted.clone();
    let vaults = RemoteVaultClient::list_vaults(&url, &after_logout.access_token).await.unwrap();
    let racing = RemoteVaultClient::new(url.clone(), vaults[0].id.clone(), "token-rusak".into(), after_logout.refresh_token)
        .with_token_persister(move |t| sink.lock().unwrap().push(t));
    racing.mark_logged_out();
    racing.list_groups().await.expect("request tetap jalan (refresh-on-401)");
    assert!(persisted.lock().unwrap().is_empty(), "token hasil rotasi setelah logout tidak boleh dipersist");
    match RemoteVaultClient::refresh_session(&url, &fresh.refresh_token).await {
        Err(VaultError::SessionExpired(_)) => {}
        other => panic!("token yang sudah di-logout harus ditolak, dapat: {:?}", other.map(|_| ())),
    }

    profil_akun(&url, &email).await;
}

/// Profil (backend/DESIGN.md Milestone 6): ganti nama, ganti email (wajib
/// password), ganti password (token lama mati, client tetap jalan pakai
/// token baru yang dipersist). Email & password DIKEMBALIKAN di akhir
/// supaya test bisa diulang.
async fn profil_akun(url: &str, email: &str) {
    let password = env_or("TERMINUS_E2E_PASSWORD", "e2e-password-123");
    let tokens = RemoteVaultClient::login(url, email, &password).await.unwrap();
    let vault_id = RemoteVaultClient::list_vaults(url, &tokens.access_token).await.unwrap()[0].id.clone();
    let persisted = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let sink = persisted.clone();
    let client = RemoteVaultClient::new(url.into(), vault_id, tokens.access_token, tokens.refresh_token.clone())
        .with_token_persister(move |t| sink.lock().unwrap().push(t));

    let info = client.update_profile(Some("  Nama E2E  "), None, None).await.expect("ganti nama");
    assert_eq!(info.full_name.as_deref(), Some("Nama E2E"), "nama di-trim server");

    let temp_email = "e2e-ganti@terminus.local";
    match client.update_profile(None, Some(temp_email), None).await {
        Err(VaultError::Remote(_)) => {}
        other => panic!("ganti email tanpa password harus ditolak, dapat: {:?}", other.map(|_| ())),
    }
    let info = client.update_profile(None, Some(temp_email), Some(&password)).await.expect("ganti email");
    assert_eq!(info.email, temp_email);

    let temp_password = "password-sementara-e2e";
    client.change_password(&password, temp_password).await.expect("ganti password");
    assert_eq!(persisted.lock().unwrap().len(), 1, "token baru hasil ganti password harus dipersist");
    match RemoteVaultClient::refresh_session(url, &tokens.refresh_token).await {
        Err(VaultError::SessionExpired(_)) => {}
        other => panic!("refresh token sebelum ganti password harus dicabut, dapat: {:?}", other.map(|_| ())),
    }
    client.list_groups().await.expect("client tetap jalan pakai token baru");

    // Kembalikan seperti semula.
    client.change_password(temp_password, &password).await.expect("kembalikan password");
    client.update_profile(None, Some(email), Some(&password)).await.expect("kembalikan email");
}
