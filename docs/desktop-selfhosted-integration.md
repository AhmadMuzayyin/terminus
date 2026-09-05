# Integrasi Desktop App: Mode Local vs Self-hosted — Desain Arsitektur

> Dokumen ini SUMBER KEBENARAN buat pekerjaan ini, sama seperti
> `backend/DESIGN.md` buat backend. WAJIB dibaca ulang tiap mau
> lanjut kerja di sini (termasuk lintas sesi) SEBELUM nulis kode Rust
> baru. Update dokumen ini kalau ada keputusan yang berubah.

## 1. Tujuan

Desktop app (`crates/`+`ui/`) SUDAH SELESAI jalan penuh di **mode
Local** (vault SQLite lokal, `crates/vault::VaultStore`, unlock pakai
master password). Tujuan pekerjaan ini: tambah **mode Self-hosted** —
user masukkan URL server (`backend/`, lihat `backend/DESIGN.md`) +
login (email+password), lalu SEMUA baca/tulis host/grup/identity/
password langsung ke server itu lewat HTTP, TANPA salinan lokal (bukan
sinkronisasi/offline-first — persis gaya Vaultwarden thin-client, sudah
disepakati waktu desain backend).

Kedua mode **saling eksklusif per-instalasi desktop**: satu app cuma
"Local" ATAU "Self-hosted" di satu waktu (pilihan tersimpan, dipakai
lagi tiap start — ganti mode = pilih ulang di layar yang sama, lihat
bagian 5).

## 2. Keputusan Desain Inti

### 2.1 Satu abstraksi `VaultBackend`, BUKAN dua alur kerja UI terpisah

`crates/app/src/state.rs` (4300+ baris) SUDAH SANGAT dalam terikat ke
`VaultStore` konkret lewat pola:
```rust
tokio::spawn(async move {
    let result = tokio::task::spawn_blocking(move || vault.lock().unwrap().save_profile(&p))
        .await...
});
```
Pola ini terulang di puluhan tempat. Menulis ulang semuanya jadi async
buat mendukung HTTP call ke server SANGAT BESAR blast radius-nya untuk
manfaat yang didapat.

**Percobaan pertama (DITOLAK, didokumentasikan biar tidak diulang)**:
pakai `reqwest::blocking::Client` di sisi Remote supaya method Remote
TETAP SINKRON — idenya biar `state.rs` tidak perlu berubah SAMA SEKALI
(cuma bungkus di `spawn_blocking` seperti method `VaultStore` yang
sudah ada). **INI PANIK DI RUNTIME**: `reqwest::blocking::Client`
mendeteksi kalau thread pemanggilnya SUDAH "milik" sebuah Tokio
runtime lalu menolak nge-drive runtime internalnya sendiri di situ
("Cannot drop a runtime in a context where blocking is not allowed") —
dan thread yang dijalankan lewat `tokio::task::spawn_blocking` MASIH
dianggap "milik" runtime yang men-spawn-nya (beda dari thread OS biasa
di luar Tokio sama sekali). Karena SEMUA pola pemanggilan `VaultStore`
di `state.rs` sekarang selalu di dalam
`tokio::spawn(async { spawn_blocking(...) })`, `reqwest::blocking`
TIDAK BISA dipakai di sana. Ditemukan & dikoreksi SEBELUM nulis kode
Milestone 2 (bukan sesudah, lihat riwayat Milestone 1 bagian 6).

**Keputusan final: `VaultBackend` method jadi `async fn`**, bukan
sinkron. Konsekuensinya LEBIH BESAR dari rencana awal — SEMUA situs
pemanggilan `vault...` di `state.rs` (~30 tempat) tetap perlu disentuh,
tapi perubahannya MEKANIS & SERAGAM (pola yang sama diulang, bukan
re-desain logic tiap tempat):

```rust
// SEBELUM (pola lama, tiap situs panggilan)
tokio::spawn(async move {
    let result = tokio::task::spawn_blocking(move || vault.lock().unwrap().save_profile(&p)).await;
    ...
});

// SESUDAH
tokio::spawn(async move {
    let result = vault.lock().await.save_profile(&p).await;
    ...
});
```

- **Tidak perlu lagi `Arc<Mutex<VaultBackend>>` di `state.rs` SAMA
  SEKALI** — sinkronisasi dipindah KE DALAM tiap varian, bukan di
  wrapper luar, supaya `VaultBackend` sendiri jadi `Clone` murah (enum
  isinya cuma `Arc<...>` per varian) dan `state.rs` cukup simpan
  `vault: VaultBackend` polos, di-`.clone()` ke tiap `tokio::spawn`
  SAMA PERSIS seperti pola `Arc::clone(&state.vault)` yang sudah ada:
  ```rust
  // crates/vault/src/lib.rs
  #[derive(Clone)]
  pub enum VaultBackend {
      Local(Arc<std::sync::Mutex<VaultStore>>), // sama seperti field `vault` LAMA di state.rs, cuma pindah ke sini
      Remote(RemoteVaultClient),                 // reqwest::Client sendiri sudah Arc-internally + Clone
  }

  impl VaultBackend {
      pub async fn save_profile(&self, profile: HostProfile) -> Result<(), VaultError> {
          match self {
              Self::Local(store) => {
                  let store = Arc::clone(store);
                  tokio::task::spawn_blocking(move || store.lock().unwrap().save_profile(&profile))
                      .await
                      .map_err(|e| VaultError::Database(format!("task panik: {e}")))?
              }
              Self::Remote(client) => client.save_profile(profile).await, // reqwest::Client ASYNC asli, TANPA spawn_blocking
          }
      }
      // ...15 method lain, pola sama.
  }
  ```
- Sisi Remote pakai `reqwest::Client` (async NATIVE, BUKAN
  `reqwest::blocking`) — HTTP I/O emang non-blocking secara alami di
  Tokio, tidak butuh `spawn_blocking` sama sekali di jalur ini.
  `RemoteVaultClient` sendiri `#[derive(Clone)]` (field `reqwest::
  Client` + `Arc<tokio::sync::Mutex<TokenPair>>` buat access/refresh
  token yang bisa berubah waktu auto-refresh, lihat 2.5).
- Situs panggilan di `state.rs` jadi lebih PENDEK dari sebelumnya
  (bukan cuma "sama"): `vault.lock().unwrap().save_profile(&p)` (di
  dalam `spawn_blocking` manual) → `vault.save_profile(p).await`
  (langsung, `spawn_blocking`-nya sudah pindah ke dalam `VaultBackend`
  di atas). `state.rs` TETAP nge-`tokio::spawn` di luar (buat tidak
  numpang task Slint) tapi TIDAK PERLU spawn_blocking manual lagi.

Kenapa ENUM, bukan `dyn Trait`: cuma 2 varian, tidak akan nambah
backend ketiga — `match` eksplisit lebih gampang dibaca & tidak butuh
`Box<dyn ...>` + object-safety compromise (termasuk buat `async fn` di
trait yang tanpa `async-trait` crate tambahan tidak object-safe sama
sekali — alasan lain ENUM lebih pas di sini). Konsisten juga dengan
gaya `ConnectionKind`/`AuthMethod` di `terminus-core` yang sudah enum.

### 2.2 Method mana yang DISERAGAMKAN, mana yang TETAP beda per-mode

**Diseragamkan** (lewat `VaultBackend`, dipanggil dari `state.rs`
TANPA tahu mode aktif) — persis 16 method CRUD yang sudah ada di
`VaultStore` sekarang:
`list_ungrouped_profiles`, `list_profiles_in_group`,
`list_all_profiles`, `save_profile`, `delete_profile`, `list_groups`,
`save_group`, `delete_group`, `store_secret`, `read_secret`,
`delete_secret`, `list_identities`, `save_identity`, `delete_identity`,
`check_host_key`, `trust_host_key`.

**TIDAK diseragamkan** (beda flow secara fundamental, ditangani di
layer startup app — lihat bagian 5, bukan lewat `VaultBackend`):
`open_default`/`open_at`/`is_initialized`/`initialize`/`unlock`/
`lock`/`is_unlocked`/`db_path` — ini semua konsep "buka file lokal +
derive key dari master password", TIDAK ADA padanannya di Self-hosted
(di situ konsepnya "login ke server dapat token", beda bentuk state-
machine-nya, maksa disamakan cuma nambah lapisan abstraksi palsu).

### 2.3 Crate baru vs tambah ke `terminus-vault`

**Keputusan: tambah ke `terminus-vault` yang sudah ada** (file baru
`src/remote.rs`), BUKAN crate terpisah. Alasan: `terminus-vault`
SUDAH JADI "lapisan penyimpanan vault" secara konsep — nambah cara
penyimpanan kedua (remote) di situ juga konsisten, dan menghindari
crate ketiga cuma buat ~1 struct + reqwest dependency (selaras gaya
proyek yang menghindari abstraksi berlebih — lihat alasan pilih
Express bukan NestJS di `backend/DESIGN.md`).

`crates/vault/Cargo.toml` nambah dependency: `reqwest` (fitur
`blocking`+`json`, TANPA `rustls`/openssl khusus — default native-tls
cukup, semua platform target sudah didukung).

### 2.4 Error handling

`VaultError` (di `crates/vault/src/lib.rs` atau file error yang sudah
ada) nambah 1 varian baru: `Remote(String)` — dipetakan dari respons
HTTP non-2xx backend (`{ "error": "..." }`, lihat `errorHandler.ts` di
backend) atau error jaringan (timeout/connection refused/dst, pesan
"Tidak bisa terhubung ke server: {detail}"). UI sudah pasti punya jalur
tampilkan error generic (dipakai buat error SQLite sekarang) — dipakai
ulang, TIDAK perlu jalur UI baru.

### 2.5 Access token & refresh token

- **Access token** (JWT, ~15 menit): disimpan IN-MEMORY SAJA di
  `RemoteVaultClient` (`Mutex<String>` atau `RwLock`). Tiap panggilan
  HTTP: kalau server balikin 401, `RemoteVaultClient` OTOMATIS panggil
  `POST /auth/refresh` pakai refresh token, dapat access token baru,
  ulangi request ASLI sekali (retry-once) — transparan buat
  `state.rs`, tidak perlu logic retry di pemanggil.
- **Refresh token** (~30 hari): HARUS persist lintas restart app
  (supaya user tidak diminta login ulang tiap buka app). **Keputusan
  final (dikonfirmasi user): disimpan di database lokal terenkripsi**,
  BUKAN plaintext, BUKAN pula OS keyring (ditolak — rapuh di Linux
  headless/WSL yang belum tentu punya secret-service daemon). Konkret:
  - File SQLite baru `<config_dir>/terminus/session.db`, SATU baris
    tabel `session_secret (id INTEGER PRIMARY KEY CHECK(id=1),
    encrypted_token BLOB NOT NULL)`.
  - Dienkripsi pakai primitive yang SUDAH ADA:
    `terminus_vault::crypto::encrypt`/`decrypt` (ChaCha20-Poly1305,
    sama seperti `secrets.data` di `vault.db`) — TIDAK nulis crypto
    baru.
  - Key enkripsinya: 32 byte random, di-generate SEKALI, disimpan di
    file sibling `<config_dir>/terminus/session.key` permission `0600`
    (Unix). **Catatan jujur/batasan**: karena key-nya juga di disk yang
    sama (supaya bisa auto-decrypt tanpa minta password lagi tiap
    start app), ini TIDAK melindungi dari penyerang yang punya akses
    penuh ke disk yang sama — perlindungannya terhadap paparan tidak
    sengaja (mis. file config ke-share/ke-backup/ke-sync ke cloud
    storage tanpa sadar isinya token, atau ter-grep waktu debugging),
    bukan terhadap compromise total mesin. Ini didokumentasikan biar
    jelas, bukan diklaim "aman total".

## 3. File Config App-level BARU

Desktop app SEKARANG TIDAK PUNYA file setting app-level sama sekali —
semua (termasuk preferensi tema per-host) disimpan DI DALAM
`vault.db`. Mode Self-hosted BUTUH tempat nyimpen "mode aktif + URL
server + refresh token" yang harus terbaca SEBELUM ada vault/koneksi
apa pun (ayam-telur: belum tentu ada `vault.db` relevan kalau mode-nya
Self-hosted).

**File baru**: `<config_dir>/terminus/app_config.json` (pakai
`directories::ProjectDirs` yang sama seperti `VaultStore::open_default`
memilih lokasi `vault.db`, jadi konsisten lintas OS).

```jsonc
{
  "mode": "local", // "local" | "self_hosted"
  "self_hosted": { // null kalau mode == "local"
    "server_url": "https://vault.contoh.com",
    "vault_id": "…" // vault yang dipakai (lihat bagian 4)
    // TIDAK ADA refresh_token di sini — itu di session.db (lihat 2.5),
    // supaya app_config.json boleh plaintext (isinya cuma URL/id, non-
    // rahasia) tanpa perlu file ini ikut dienkripsi seluruhnya.
  }
}
```

Modul baru `crates/app/src/app_config.rs`: `load()` (default ke
`{mode: "local"}` kalau file belum ada — INI PENTING, instalasi LAMA
yang upgrade otomatis tetap mode Local tanpa perubahan perilaku),
`save(&AppConfig)`. Modul baru `crates/app/src/session_store.rs`
(atau di `terminus-vault`, lihat bagian 6 milestone 1): kelola
`session.db`+`session.key` (bagian 2.5) — `save_refresh_token(&str)`/
`load_refresh_token() -> Option<String>`/`clear()` (dipanggil waktu
logout).

## 4. Konsep "Vault" Ganda: Desktop (1 file = 1 vault implisit) vs Backend (banyak vault per user)

Backend API mendukung SATU user jadi anggota BANYAK vault (personal +
tim, lihat `backend/DESIGN.md` bagian 4). Desktop app SEKARANG hanya
punya konsep SATU vault implisit (`vault.db`) — tidak ada UI "pilih
vault".

**Keputusan v1 (jangan bikin vault-switcher UI dulu, itu fitur
terpisah)**:
- Sesudah login sukses, desktop app panggil `GET /api/v1/vaults`:
  - **0 vault** → otomatis `POST /api/v1/vaults` bikin satu bernama
    "My Vault" (mirror pengalaman "vault.db baru otomatis dibikin"
    waktu first-run mode Local).
  - **1 vault** → langsung dipakai, transparan (paling umum, cocok
    sama mental model desktop yang sudah ada).
  - **>1 vault** (skenario tim) → tampilkan daftar sederhana (bukan
    switcher penuh) SEKALI waktu login, user pilih satu → `vault_id`
    itu disimpan di `app_config.json` dan dipakai terus tiap start
    SAMPAI user logout (ganti pilihan = logout dulu). Ganti vault
    mid-session TANPA logout eksplisit DILUAR SCOPE v1.

## 5. Alur UI

Titik integrasi: `VaultDialog` (`ui/components/dialogs.slint`) —
SATU-SATUNYA layar yang tampil SEBELUM `VaultModel.is-unlocked`
(lihat `ui/app-window.slint` baris ~92), jadi tempat paling pas buat
pilihan mode (tidak perlu halaman Settings global baru yang belum ada
sama sekali di app ini sekarang).

`VaultDialog` ditambah 1 tab-switcher di atas ("Local" | "Self-hosted"):
- **Tab Local** — isi PERSIS SAMA seperti sekarang (create/unlock
  master password), TIDAK ADA PERUBAHAN PERILAKU.
- **Tab Self-hosted** — field Server URL + Email + Password + tombol
  "Masuk". Link kecil "Server baru? Daftar admin pertama" toggle ke
  form Register (Email+Password+Konfirmasi) yang manggil
  `POST /auth/register` (cuma sukses kalau tabel `users` backend masih
  kosong, lihat `backend/DESIGN.md` 5.2 — error-nya ditampilkan apa
  adanya kalau sudah ada admin).
- Tab yang aktif TERAKHIR KALI dipakai diingat lewat `app_config.json`
  (`mode`), jadi user tidak perlu pilih ulang tiap buka app.
- **Ganti mode** (Local→Self-hosted atau sebaliknya): user klik tab
  lain lagi di layar ini — TAPI cuma bisa dilakukan dari layar
  VaultDialog ini sendiri, artinya harus dalam keadaan "belum unlock/
  belum login" dulu (lock/logout). Tidak ada tombol "ganti mode" di
  tengah sesi yang sedang unlocked — konsisten dengan tidak adanya
  halaman Settings global, dan menghindari kerumitan "swap backend
  storage di tengah sesi yang masih ada tab terminal aktif".

`main.rs` startup logic baru:
```
config = app_config::load()
match config.mode {
  Local => // PERSIS alur sekarang, tidak berubah
    VaultStore::open_default() -> VaultBackend::Local(..)
  SelfHosted => // BARU
    tampilkan tab Self-hosted duluan di VaultDialog (bukan Local),
    prefill Server URL dari config kalau ada, refresh_token kalau ada
    dicoba dulu (silent) sebelum minta password lagi
}
```

## 6. Urutan Pengerjaan (Milestone)

Dikerjakan SATU-SATU (konsisten cara kerja backend), tiap milestone
diverifikasi (`cargo build --workspace` + `cargo test --workspace` +
`slint-viewer --check`) sebelum lanjut:

1. ✅ **Fondasi**: `crates/app/src/app_config.rs` (load/save + default
   Local, path lewat `directories`), `crates/app/src/session_store.rs`
   (`session.db`+`session.key` terenkripsi via `terminus_vault::crypto`
   yang sudah ada, lihat bagian 2.5), `VaultError::Remote` varian baru
   di `crates/vault/src/lib.rs`. Dependency baru: `reqwest` (workspace
   + `crates/vault`, versi ASYNC BUKAN `blocking` — lihat koreksi
   desain di bagian 2.1, ditemukan & diperbaiki DI MILESTONE INI
   sebelum `RemoteVaultClient` ditulis), `tokio` di `crates/vault`
   (buat `spawn_blocking` internal Milestone 2), `serde`/`serde_json`/
   `directories`/`rusqlite`/`rand` di `crates/app` (belum pernah
   dipakai crate itu sebelumnya). Kedua modul baru didaftarkan di
   `main.rs` tapi BELUM dipanggil (`#![allow(dead_code)]` sementara,
   dihapus waktu wiring Milestone 3) — diverifikasi
   `cargo build --workspace` bersih + `cargo test -p terminus-app`
   13/13 lulus (termasuk 3 test baru: default config mode Local,
   roundtrip JSON, roundtrip crypto key session).
2. ⏳ **`RemoteVaultClient` + `VaultBackend` enum**: implementasi 16
   method CRUD (blocking HTTP + retry-on-401 refresh), unit test pakai
   mock HTTP server (`wiremock` atau server Express asli di
   `backend/` jalan background — DIPUTUSKAN waktu dikerjakan). Ganti
   `state.rs` field `vault: Arc<Mutex<VaultStore>>` jadi
   `Arc<Mutex<VaultBackend>>` — verifikasi call site lama TETAP
   compile tanpa ubah logic (cuma ganti tipe deklarasi + wiring
   startup).
3. ⏳ **UI `VaultDialog` tab Local/Self-hosted** + wiring
   login/register/refresh-token-silent-login di `main.rs`/`state.rs`.
4. ⏳ **Resolusi vault (bagian 4)**: auto-create/auto-select/pilih
   vault sesudah login, simpan `vault_id` ke `app_config.json`.
5. ⏳ **Verifikasi end-to-end manual**: jalankan `backend/` lewat
   `npm run dev` + MySQL lokal, desktop app mode Self-hosted connect
   ke situ — create host/grup/identity, set/get password, lock+buka
   lagi (refresh token jalan), ganti balik ke mode Local (data lokal
   lama tidak keganggu).

## 7. Sengaja DI LUAR SCOPE (jangan dikerjakan tanpa diminta)

- Vault switcher mid-session (ganti vault tanpa logout).
- Sinkronisasi/cache offline utk mode Self-hosted (SENGAJA thin-client
  murni, lihat bagian 1).
- Halaman Settings app-level umum (kalau nanti dibutuhkan fitur lain
  yang butuh itu, baru dibikin — v1 numpang di `VaultDialog` saja).
- Invite member vault dari desktop app (backend sudah punya endpoint-
  nya, UI-nya belum, fitur terpisah).
- Android app.
