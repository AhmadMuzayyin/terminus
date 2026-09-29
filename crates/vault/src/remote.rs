//! Client HTTP mode Self-hosted — implementasi konkret sisi `Remote`
//! di `VaultBackend` (lihat `lib.rs`). Lihat
//! `docs/desktop-selfhosted-integration.md` bagian 2 buat rasional
//! desain lengkap, KHUSUSNYA bagian 2.6 (kenapa `credential_id` di
//! sini SELALU disamakan dengan `id` resource pemiliknya, dan kenapa
//! ada `pending_secrets` buffer) dan 2.7 (kenapa `check_host_key`/
//! `trust_host_key` TIDAK ADA di sini sama sekali).
//!
//! SEMUA method di sini `async` NATIVE (`reqwest::Client`, BUKAN
//! `reqwest::blocking` — lihat penjelasan panjang di bagian 2.1 doc di
//! atas soal kenapa `reqwest::blocking` panik kalau dipanggil dari
//! `tokio::task::spawn_blocking`).

use std::collections::HashMap;
use std::sync::Arc;

use reqwest::{Method, StatusCode};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};
use terminus_core::{AuthMethod, ConnectionKind, HostGroup, HostProfile, Identity};
use tokio::sync::Mutex as TokioMutex;
use uuid::Uuid;

use crate::VaultError;

/// Pasangan token hasil login/register/refresh. Publik (beda dari
/// `TokenPair` internal) karena layer app perlu menyimpan
/// `refresh_token`-nya (`session_store`) sebelum membangun client.
#[derive(Debug, Clone)]
pub struct AuthTokens {
    pub access_token: String,
    pub refresh_token: String,
}

/// Satu baris `GET /api/v1/vaults` (role sengaja tidak dibaca — v1
/// desktop tidak membedakan owner/member, lihat doc bagian 4).
#[derive(Debug, Clone, Deserialize)]
pub struct VaultSummary {
    pub id: String,
    pub name: String,
}

/// Dipanggil (di `spawn_blocking`) tiap refresh token BARU keluar dari
/// rotasi otomatis di `refresh_access_token` — backend mencabut token
/// lama begitu dipakai, jadi kalau yang baru tidak dipersist, login
/// diam-diam di restart berikutnya pasti gagal.
type TokenPersister = Arc<dyn Fn(String) + Send + Sync>;

#[derive(Clone)]
struct TokenPair {
    access_token: String,
    refresh_token: String,
}

/// Client HTTP ke satu vault tertentu di satu server backend tertentu.
/// `#[derive(Clone)]` SENGAJA murah (`Arc` di dalam) — lihat
/// `VaultBackend` di `lib.rs`, tidak ada `Arc<Mutex<RemoteVaultClient>>`
/// di luar, sinkronisasi cukup di dalam sini per-field yang memang
/// berubah (`tokens`, `pending_secrets`).
#[derive(Clone)]
pub struct RemoteVaultClient {
    http: reqwest::Client,
    base_url: String,
    vault_id: String,
    tokens: Arc<TokioMutex<TokenPair>>,
    /// Password yang sudah di-`store_secret` TAPI resource pemiliknya
    /// (host/identity) BELUM ADA di server — dikirim beneran waktu
    /// `save_profile`/`save_identity` berikutnya. Lihat bagian 2.6 doc.
    pending_secrets: Arc<TokioMutex<HashMap<Uuid, Vec<u8>>>>,
    on_tokens_rotated: Option<TokenPersister>,
    /// Di-set `logout()` PALING AWAL — request yang lagi jalan di
    /// background bisa saja me-refresh token SETELAH logout; hasil
    /// rotasinya TIDAK BOLEH dipersist lagi (kalau dipersist, start app
    /// berikutnya malah masuk otomatis padahal user sudah logout).
    logged_out: Arc<std::sync::atomic::AtomicBool>,
}

/// Timeout request auth (login/register/refresh/list vault) — tanpa ini
/// server yang hang bikin dialog login `busy` selamanya.
const AUTH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

impl RemoteVaultClient {
    pub fn new(base_url: String, vault_id: String, access_token: String, refresh_token: String) -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url,
            vault_id,
            tokens: Arc::new(TokioMutex::new(TokenPair { access_token, refresh_token })),
            pending_secrets: Arc::new(TokioMutex::new(HashMap::new())),
            on_tokens_rotated: None,
            logged_out: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Pasang callback persist refresh token hasil rotasi otomatis.
    pub fn with_token_persister(mut self, persister: impl Fn(String) + Send + Sync + 'static) -> Self {
        self.on_tokens_rotated = Some(Arc::new(persister));
        self
    }

    fn auth_http() -> Result<reqwest::Client, VaultError> {
        reqwest::Client::builder()
            .timeout(AUTH_TIMEOUT)
            .build()
            .map_err(|e| VaultError::Remote(format!("gagal menyiapkan HTTP client: {e}")))
    }

    async fn auth_call(base_url: &str, endpoint: &str, body: Value) -> Result<AuthTokens, VaultError> {
        let resp = Self::auth_http()?
            .post(format!("{base_url}/api/v1/auth/{endpoint}"))
            .json(&body)
            .send()
            .await
            .map_err(Self::network_err)?;
        Self::tokens_from_response(resp).await
    }

    async fn tokens_from_response(resp: reqwest::Response) -> Result<AuthTokens, VaultError> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Body {
            access_token: String,
            refresh_token: String,
        }
        let body: Body = Self::json_or_err(resp).await?;
        Ok(AuthTokens { access_token: body.access_token, refresh_token: body.refresh_token })
    }

    /// `POST /auth/login`. Respons juga memuat `user`, tidak dipakai.
    pub async fn login(base_url: &str, email: &str, password: &str) -> Result<AuthTokens, VaultError> {
        Self::auth_call(base_url, "login", json!({ "email": email, "password": password })).await
    }

    /// `POST /auth/register` — backend cuma menerima kalau tabel `users`
    /// masih kosong (admin pertama), error-nya diteruskan apa adanya.
    pub async fn register(base_url: &str, email: &str, password: &str) -> Result<AuthTokens, VaultError> {
        Self::auth_call(base_url, "register", json!({ "email": email, "password": password })).await
    }

    /// Login diam-diam pakai refresh token tersimpan. Status 401 dari
    /// server -> `SessionExpired` (token harus dibuang); kegagalan lain
    /// (jaringan, 5xx) tetap `Remote` supaya token tersimpan TIDAK
    /// dibuang cuma karena server sedang mati.
    pub async fn refresh_session(base_url: &str, refresh_token: &str) -> Result<AuthTokens, VaultError> {
        let resp = Self::auth_http()?
            .post(format!("{base_url}/api/v1/auth/refresh"))
            .json(&json!({ "refreshToken": refresh_token }))
            .send()
            .await
            .map_err(Self::network_err)?;
        if resp.status() == StatusCode::UNAUTHORIZED {
            return Err(match Self::error_from_response(resp).await {
                VaultError::Remote(msg) => VaultError::SessionExpired(msg),
                other => other,
            });
        }
        Self::tokens_from_response(resp).await
    }

    /// Email akun yang lagi login (`GET /auth/me`) — ditampilkan di
    /// sidebar di samping tombol Logout, juga waktu login diam-diam (di
    /// situ email tidak diketik user).
    pub async fn me(base_url: &str, access_token: &str) -> Result<String, VaultError> {
        #[derive(Deserialize)]
        struct Me {
            email: String,
        }
        let resp = Self::auth_http()?
            .get(format!("{base_url}/api/v1/auth/me"))
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(Self::network_err)?;
        Ok(Self::json_or_err::<Me>(resp).await?.email)
    }

    /// Stop persist token hasil rotasi mulai SEKARANG (sinkron). Dipanggil
    /// app sebelum menghapus token tersimpan; `logout()` juga memanggilnya.
    pub fn mark_logged_out(&self) {
        self.logged_out.store(true, std::sync::atomic::Ordering::SeqCst);
    }

    /// Cabut refresh token TERBARU di server (`POST /auth/logout`). Backend
    /// selalu 204 walau token sudah dicabut duluan, jadi error di sini
    /// praktis cuma jaringan — caller menganggapnya best-effort.
    pub async fn logout(&self) -> Result<(), VaultError> {
        self.mark_logged_out();
        let refresh_token = self.current_refresh_token().await;
        let resp = Self::auth_http()?
            .post(format!("{}/api/v1/auth/logout", self.base_url))
            .json(&json!({ "refreshToken": refresh_token }))
            .send()
            .await
            .map_err(Self::network_err)?;
        Self::empty_or_err(resp).await
    }

    pub async fn list_vaults(base_url: &str, access_token: &str) -> Result<Vec<VaultSummary>, VaultError> {
        let resp = Self::auth_http()?
            .get(format!("{base_url}/api/v1/vaults"))
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(Self::network_err)?;
        Self::json_or_err(resp).await
    }

    pub async fn create_vault(base_url: &str, access_token: &str, name: &str) -> Result<VaultSummary, VaultError> {
        let resp = Self::auth_http()?
            .post(format!("{base_url}/api/v1/vaults"))
            .bearer_auth(access_token)
            .json(&json!({ "name": name }))
            .send()
            .await
            .map_err(Self::network_err)?;
        Self::json_or_err(resp).await
    }

    /// Snapshot refresh token TERBARU (bisa sudah rotate dari nilai
    /// awal waktu `new()`, lihat `refresh_access_token`) — dipakai
    /// `logout` (token yang dicabut harus yang TERBARU).
    pub async fn current_refresh_token(&self) -> String {
        self.tokens.lock().await.refresh_token.clone()
    }

    fn vault_url(&self, path: &str) -> String {
        format!("{}/api/v1/vaults/{}{}", self.base_url, self.vault_id, path)
    }

    /// Kirim satu request ber-auth, transparan tangani refresh-on-401
    /// (retry SEKALI). Response HTTP status APA PUN (termasuk 4xx)
    /// dikembalikan sebagai `Ok` — caller yang putuskan (mis.
    /// `save_profile` sengaja cek status 404 dulu sebelum menganggapnya
    /// error, lihat bagian 2.6 doc). Cuma kegagalan JARINGAN (timeout/
    /// connection refused/dst) yang jadi `Err` di sini.
    async fn send(&self, method: Method, path: &str, body: Option<Value>) -> Result<reqwest::Response, VaultError> {
        let url = self.vault_url(path);
        let access_token = self.tokens.lock().await.access_token.clone();
        let resp = self.build_request(&method, &url, &body, &access_token).send().await.map_err(Self::network_err)?;

        if resp.status() != StatusCode::UNAUTHORIZED {
            return Ok(resp);
        }

        // Access token kedaluwarsa (~15 menit, lihat backend/DESIGN.md
        // bagian 2) -- refresh SEKALI lalu ulangi request ASLI, transparan
        // buat semua caller (tidak perlu logic retry di 16 method CRUD).
        self.refresh_access_token().await?;
        let access_token = self.tokens.lock().await.access_token.clone();
        self.build_request(&method, &url, &body, &access_token).send().await.map_err(Self::network_err)
    }

    fn build_request(&self, method: &Method, url: &str, body: &Option<Value>, access_token: &str) -> reqwest::RequestBuilder {
        let req = self.http.request(method.clone(), url).bearer_auth(access_token);
        match body {
            Some(b) => req.json(b),
            None => req,
        }
    }

    fn network_err(e: reqwest::Error) -> VaultError {
        VaultError::Remote(format!("tidak bisa terhubung ke server: {e}"))
    }

    async fn refresh_access_token(&self) -> Result<(), VaultError> {
        let refresh_token = self.tokens.lock().await.refresh_token.clone();
        let url = format!("{}/api/v1/auth/refresh", self.base_url);
        let resp = self
            .http
            .post(&url)
            .json(&json!({ "refreshToken": refresh_token }))
            .send()
            .await
            .map_err(Self::network_err)?;

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct RefreshResult {
            access_token: String,
            refresh_token: String,
        }
        let result: RefreshResult = Self::json_or_err(resp).await.map_err(|e| match e {
            // Refresh token sendiri sudah revoked/kedaluwarsa (mis. app
            // tidak dibuka lama sekali, lihat `REFRESH_TOKEN_TTL_SECONDS`
            // backend) -- user WAJIB login ulang manual, bukan sekadar
            // error jaringan biasa.
            VaultError::Remote(msg) => VaultError::Remote(format!("sesi berakhir, login ulang: {msg}")),
            other => other,
        })?;

        let new_refresh_token = {
            let mut tokens = self.tokens.lock().await;
            tokens.access_token = result.access_token;
            tokens.refresh_token = result.refresh_token;
            tokens.refresh_token.clone()
        };
        let logged_out = self.logged_out.load(std::sync::atomic::Ordering::SeqCst);
        if let (Some(persist), false) = (&self.on_tokens_rotated, logged_out) {
            let persist = Arc::clone(persist);
            // Hasil join diabaikan: gagal persist tidak boleh
            // menggagalkan request user yang refresh-nya SUDAH sukses
            // (persister sendiri yang log error-nya).
            let _ = tokio::task::spawn_blocking(move || persist(new_refresh_token)).await;
        }
        Ok(())
    }

    async fn error_from_response(resp: reqwest::Response) -> VaultError {
        let status = resp.status();
        let message = resp
            .json::<Value>()
            .await
            .ok()
            .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(|s| s.to_string()))
            .unwrap_or_else(|| format!("HTTP {status}"));
        VaultError::Remote(message)
    }

    async fn json_or_err<T: DeserializeOwned>(resp: reqwest::Response) -> Result<T, VaultError> {
        if !resp.status().is_success() {
            return Err(Self::error_from_response(resp).await);
        }
        resp.json::<T>().await.map_err(|e| VaultError::Remote(format!("respons server tidak terduga: {e}")))
    }

    async fn empty_or_err(resp: reqwest::Response) -> Result<(), VaultError> {
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(Self::error_from_response(resp).await)
        }
    }

    /// Kirim plaintext yang sudah dibuffer (kalau ada) buat
    /// `credential_id` ini ke `PUT {prefix}/:resource_id/secret`, lalu
    /// hapus dari buffer. Dipanggil dari `save_profile` SETELAH host-nya
    /// sukses dibuat/diupdate. Lihat bagian 2.6 doc.
    ///
    /// Buffer di-key `credential_id`, tapi URL WAJIB pakai `resource_id`
    /// (id host): host BARU dari `state.rs` punya `credential_id` UUID
    /// terpisah yang tidak pernah dikenal server — `PUT` ke situ 404
    /// (ditemukan E2E Milestone 6, `tests/remote_e2e.rs`).
    async fn flush_pending_secret(&self, credential_id: Uuid, prefix: &str, resource_id: Uuid) -> Result<(), VaultError> {
        let maybe_plaintext = self.pending_secrets.lock().await.remove(&credential_id);
        let Some(plaintext) = maybe_plaintext else { return Ok(()) };
        let password = Self::plaintext_to_password_string(plaintext)?;
        let resp = self
            .send(Method::PUT, &format!("{prefix}/{resource_id}/secret"), Some(json!({ "password": password })))
            .await?;
        Self::empty_or_err(resp).await
    }

    fn plaintext_to_password_string(plaintext: Vec<u8>) -> Result<String, VaultError> {
        String::from_utf8(plaintext).map_err(|_| {
            VaultError::Remote("password bukan teks UTF-8 valid — mode Self-hosted cuma dukung password teks, bukan private key biner".into())
        })
    }

    // ------------------------------------------------------------
    // Host profile CRUD
    // ------------------------------------------------------------

    pub async fn list_all_profiles(&self) -> Result<Vec<HostProfile>, VaultError> {
        let resp = self.send(Method::GET, "/hosts", None).await?;
        let dtos: Vec<HostDto> = Self::json_or_err(resp).await?;
        dtos.into_iter().map(HostDto::into_profile).collect()
    }

    /// Backend tidak punya endpoint filter server-side terpisah buat
    /// ini — vault self-hosted TIDAK diharapkan punya host sebanyak
    /// itu sampai filter client-side jadi masalah performa nyata.
    pub async fn list_ungrouped_profiles(&self) -> Result<Vec<HostProfile>, VaultError> {
        Ok(self.list_all_profiles().await?.into_iter().filter(|p| p.group_id.is_none()).collect())
    }

    pub async fn list_profiles_in_group(&self, group_id: Uuid) -> Result<Vec<HostProfile>, VaultError> {
        Ok(self.list_all_profiles().await?.into_iter().filter(|p| p.group_id == Some(group_id)).collect())
    }

    /// Upsert: coba `PUT` (update) dulu, kalau 404 (belum ada di
    /// server) baru `POST` (create, sertakan `id` — lihat
    /// backend/DESIGN.md addendum Milestone 5). Sesudah sukses, kirim
    /// password yang sudah dibuffer (kalau ada, lihat bagian 2.6 doc).
    pub async fn save_profile(&self, profile: &HostProfile) -> Result<(), VaultError> {
        let body = HostDto::to_update_body(profile);
        let put_resp = self.send(Method::PUT, &format!("/hosts/{}", profile.id), Some(body.clone())).await?;
        if put_resp.status() == StatusCode::NOT_FOUND {
            let mut create_body = body;
            create_body["id"] = json!(profile.id.to_string());
            let create_resp = self.send(Method::POST, "/hosts", Some(create_body)).await?;
            Self::empty_or_err(create_resp).await?;
        } else {
            Self::empty_or_err(put_resp).await?;
        }

        let credential_id = match profile.auth {
            AuthMethod::Password { credential_id } => credential_id,
            _ => profile.id, // AuthMethod::Agent/PrivateKey belum didukung mode Self-hosted, fallback aman.
        };
        self.flush_pending_secret(credential_id, "/hosts", profile.id).await
    }

    pub async fn delete_profile(&self, id: Uuid) -> Result<(), VaultError> {
        let resp = self.send(Method::DELETE, &format!("/hosts/{id}"), None).await?;
        self.pending_secrets.lock().await.remove(&id);
        Self::empty_or_err(resp).await
    }

    // ------------------------------------------------------------
    // Grup
    // ------------------------------------------------------------

    pub async fn list_groups(&self) -> Result<Vec<HostGroup>, VaultError> {
        let resp = self.send(Method::GET, "/groups", None).await?;
        let dtos: Vec<GroupDto> = Self::json_or_err(resp).await?;
        dtos.into_iter().map(GroupDto::into_group).collect()
    }

    pub async fn save_group(&self, group: &HostGroup) -> Result<(), VaultError> {
        let body = GroupDto::to_update_body(group);
        let put_resp = self.send(Method::PUT, &format!("/groups/{}", group.id), Some(body.clone())).await?;
        if put_resp.status() == StatusCode::NOT_FOUND {
            let mut create_body = body;
            create_body["id"] = json!(group.id.to_string());
            let create_resp = self.send(Method::POST, "/groups", Some(create_body)).await?;
            Self::empty_or_err(create_resp).await
        } else {
            Self::empty_or_err(put_resp).await
        }
    }

    pub async fn delete_group(&self, id: Uuid) -> Result<(), VaultError> {
        let resp = self.send(Method::DELETE, &format!("/groups/{id}"), None).await?;
        Self::empty_or_err(resp).await
    }

    // ------------------------------------------------------------
    // Secret (password host/identity) — lihat bagian 2.6 doc buat
    // penjelasan lengkap urutan buffer-lalu-flush ini.
    // ------------------------------------------------------------

    pub async fn store_secret(&self, credential_id: Uuid, plaintext: &[u8]) -> Result<(), VaultError> {
        let password = Self::plaintext_to_password_string(plaintext.to_vec())?;
        let resp = self
            .send(Method::PUT, &format!("/hosts/{credential_id}/secret"), Some(json!({ "password": password })))
            .await?;
        if resp.status() == StatusCode::NOT_FOUND {
            // Host (ATAU identity, belum tahu yang mana di titik ini)
            // dengan id ini belum ada di server -- buffer, dikirim
            // beneran oleh `save_profile`/`save_identity` berikutnya.
            self.pending_secrets.lock().await.insert(credential_id, plaintext.to_vec());
            return Ok(());
        }
        Self::empty_or_err(resp).await
    }

    /// Coba endpoint host dulu, fallback ke identity kalau 404 --
    /// `credential_id` generik di sini bisa merujuk salah satu (lihat
    /// bagian 2.6 doc, tidak ada cara lain tahu duluan tanpa
    /// menyimpan state tambahan).
    pub async fn read_secret(&self, credential_id: Uuid) -> Result<Vec<u8>, VaultError> {
        let host_resp = self.send(Method::GET, &format!("/hosts/{credential_id}/secret"), None).await?;
        if host_resp.status() != StatusCode::NOT_FOUND {
            let dto: SecretDto = Self::json_or_err(host_resp).await?;
            return Ok(dto.password.into_bytes());
        }
        let identity_resp = self.send(Method::GET, &format!("/identities/{credential_id}/secret"), None).await?;
        let dto: SecretDto = Self::json_or_err(identity_resp).await?;
        Ok(dto.password.into_bytes())
    }

    /// Backend TIDAK punya endpoint hapus secret independen dari
    /// resource pemiliknya — `delete_profile`/`delete_group`/
    /// `delete_identity` di backend SUDAH cascade hapus baris
    /// `secrets`-nya sendiri (lihat *.service.ts). Method generik ini
    /// TIDAK PERNAH dipanggil standalone dari `state.rs` (dicek waktu
    /// desain, lihat bagian 2.6 doc) — no-op selain beberes buffer.
    pub async fn delete_secret(&self, credential_id: Uuid) -> Result<(), VaultError> {
        self.pending_secrets.lock().await.remove(&credential_id);
        Ok(())
    }

    // ------------------------------------------------------------
    // Identity — BEDA dari host: password WAJIB ikut di request CREATE
    // yang sama (backend tidak punya create-tanpa-password buat
    // identity), lihat bagian 2.6 doc.
    // ------------------------------------------------------------

    pub async fn list_identities(&self) -> Result<Vec<Identity>, VaultError> {
        let resp = self.send(Method::GET, "/identities", None).await?;
        let dtos: Vec<IdentityDto> = Self::json_or_err(resp).await?;
        dtos.into_iter().map(IdentityDto::into_identity).collect()
    }

    pub async fn save_identity(&self, identity: &Identity) -> Result<(), VaultError> {
        let update_body = json!({ "label": identity.label, "username": identity.username });
        let put_resp = self.send(Method::PUT, &format!("/identities/{}", identity.id), Some(update_body)).await?;
        if put_resp.status() != StatusCode::NOT_FOUND {
            return Self::empty_or_err(put_resp).await;
        }

        let plaintext = self.pending_secrets.lock().await.remove(&identity.credential_id);
        let Some(plaintext) = plaintext else {
            // Tidak seharusnya kejadian — `state.rs` SELALU panggil
            // `store_secret(identity.credential_id, ...)` SEBELUM
            // `save_identity` (lihat bagian 2.6 doc). Jaga-jaga
            // eksplisit daripada kirim create tanpa password (backend
            // bakal nolak 400 dengan pesan yang lebih membingungkan).
            return Err(VaultError::Remote(
                "identity baru butuh password (store_secret harus dipanggil sebelum save_identity)".into(),
            ));
        };
        let password = Self::plaintext_to_password_string(plaintext)?;
        let create_body = json!({
            "id": identity.id.to_string(),
            "label": identity.label,
            "username": identity.username,
            "password": password,
        });
        let create_resp = self.send(Method::POST, "/identities", Some(create_body)).await?;
        Self::empty_or_err(create_resp).await
    }

    pub async fn delete_identity(&self, id: Uuid) -> Result<(), VaultError> {
        let resp = self.send(Method::DELETE, &format!("/identities/{id}"), None).await?;
        self.pending_secrets.lock().await.remove(&id);
        Self::empty_or_err(resp).await
    }
}

// ------------------------------------------------------------
// DTO <-> terminus_core mapping. `#[serde(rename_all = "camelCase")]`
// di semua DTO respons — field backend SEMUA camelCase (lihat
// backend/prisma/schema.prisma & */*.service.ts).
// ------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HostDto {
    id: String,
    label: String,
    host: String,
    port: u16,
    username: String,
    kind: String,
    group_id: Option<String>,
    tags: Vec<String>,
    terminal_theme: Option<String>,
    // `hasPassword` sengaja tidak dipakai di sisi Rust (state.rs tidak
    // pernah butuh field ini secara terpisah dari `read_secret` gagal/
    // sukses) — dibiarkan di struct biar match field backend APA
    // ADANYA, ditandai supaya tidak muncul warning dead_code.
    #[allow(dead_code)]
    has_password: bool,
}

impl HostDto {
    fn into_profile(self) -> Result<HostProfile, VaultError> {
        let id = Uuid::parse_str(&self.id).map_err(|e| VaultError::Remote(format!("id host dari server tidak valid: {e}")))?;
        Ok(HostProfile {
            id,
            label: self.label,
            host: self.host,
            port: self.port,
            username: self.username,
            kind: if self.kind == "cisco_ios" { ConnectionKind::CiscoIos } else { ConnectionKind::Ssh },
            // Lihat bagian 2.6 doc: backend tidak expose credential_id
            // terpisah, id host ITU SENDIRI dipakai sebagai
            // credential_id di sisi client.
            auth: AuthMethod::Password { credential_id: id },
            group_id: self.group_id.and_then(|s| Uuid::parse_str(&s).ok()),
            tags: self.tags,
            terminal_theme: self.terminal_theme,
        })
    }

    fn to_update_body(profile: &HostProfile) -> Value {
        let kind = match profile.kind {
            ConnectionKind::CiscoIos => "cisco_ios",
            ConnectionKind::Ssh => "ssh",
        };
        json!({
            "label": profile.label,
            "host": profile.host,
            "port": profile.port,
            "username": profile.username,
            "kind": kind,
            "groupId": profile.group_id.map(|g| g.to_string()),
            "tags": profile.tags,
            "terminalTheme": profile.terminal_theme,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GroupDto {
    id: String,
    name: String,
    subtitle: Option<String>,
    parent_id: Option<String>,
}

impl GroupDto {
    fn into_group(self) -> Result<HostGroup, VaultError> {
        Ok(HostGroup {
            id: Uuid::parse_str(&self.id).map_err(|e| VaultError::Remote(format!("id grup dari server tidak valid: {e}")))?,
            name: self.name,
            subtitle: self.subtitle,
            parent_id: self.parent_id.and_then(|s| Uuid::parse_str(&s).ok()),
        })
    }

    fn to_update_body(group: &HostGroup) -> Value {
        json!({
            "name": group.name,
            "subtitle": group.subtitle,
            "parentId": group.parent_id.map(|p| p.to_string()),
        })
    }
}

#[derive(Debug, Deserialize)]
struct IdentityDto {
    id: String,
    label: String,
    username: String,
}

impl IdentityDto {
    fn into_identity(self) -> Result<Identity, VaultError> {
        let id = Uuid::parse_str(&self.id).map_err(|e| VaultError::Remote(format!("id identity dari server tidak valid: {e}")))?;
        // Lihat bagian 2.6 doc: sama seperti host, id identity ITU
        // SENDIRI dipakai sebagai credential_id di sisi client.
        Ok(Identity { id, label: self.label, username: self.username, credential_id: id })
    }
}

#[derive(Debug, Deserialize)]
struct SecretDto {
    password: String,
}
