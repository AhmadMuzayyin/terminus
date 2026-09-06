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
}

impl RemoteVaultClient {
    pub fn new(base_url: String, vault_id: String, access_token: String, refresh_token: String) -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url,
            vault_id,
            tokens: Arc::new(TokioMutex::new(TokenPair { access_token, refresh_token })),
            pending_secrets: Arc::new(TokioMutex::new(HashMap::new())),
        }
    }

    /// Snapshot refresh token TERBARU (bisa sudah rotate dari nilai
    /// awal waktu `new()`, lihat `refresh_access_token`) — dipanggil
    /// Milestone 3 buat persist ulang ke `session_store` supaya login
    /// diam-diam (silent) tetap valid di restart app berikutnya.
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

        let mut tokens = self.tokens.lock().await;
        tokens.access_token = result.access_token;
        tokens.refresh_token = result.refresh_token;
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
    /// `credential_id` ini ke `PUT {prefix}/:credential_id/secret`,
    /// lalu hapus dari buffer. Dipanggil dari `save_profile` SETELAH
    /// host-nya sukses dibuat/diupdate. Lihat bagian 2.6 doc.
    async fn flush_pending_secret(&self, credential_id: Uuid, prefix: &str) -> Result<(), VaultError> {
        let maybe_plaintext = self.pending_secrets.lock().await.remove(&credential_id);
        let Some(plaintext) = maybe_plaintext else { return Ok(()) };
        let password = Self::plaintext_to_password_string(plaintext)?;
        let resp = self
            .send(Method::PUT, &format!("{prefix}/{credential_id}/secret"), Some(json!({ "password": password })))
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
        self.flush_pending_secret(credential_id, "/hosts").await
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
