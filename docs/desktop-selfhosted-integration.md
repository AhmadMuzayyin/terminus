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

### 2.6 Ketidakcocokan ID: `state.rs` client-authoritative vs REST server-authoritative (DITEMUKAN & DIPERBAIKI sebelum nulis `RemoteVaultClient`)

`state.rs` SELALU generate `id` (host/grup/identity) DAN `credential_id`
(terpisah) SENDIRI sebelum memanggil `save_profile`/`save_identity` —
lalu memanggil `store_secret(credential_id, plaintext)` **SEBELUM**
`save_profile`/`save_identity` (aman di SQLite lokal, tidak ada FK).
Backend, sebaliknya, SELALU generate `id`-nya sendiri di `POST
/hosts|groups|identities` (sebelum perbaikan ini client tidak bisa
kirim `id` sama sekali) DAN tidak punya endpoint "set secret" yang
independen dari resource induknya (`PUT /hosts/:id/secret` butuh host
itu SUDAH ADA; `POST /identities` malah MEWAJIBKAN password di request
yang SAMA, tidak ada endpoint set-secret identity sama sekali).

**Perbaikan 1 (backend, kecil & backward-compatible)**: `POST
/vaults/:vaultId/{hosts,groups,identities}` sekarang terima field `id`
OPSIONAL di body — kalau dikirim, dipakai APA ADANYA sebagai primary
key. Lihat `backend/DESIGN.md` addendum Milestone 5. Ini menghilangkan
setengah masalah: `id` client SEKARANG SELALU sama dengan `id` server,
tidak ada lagi rekonsiliasi id.

**Perbaikan 2 (`RemoteVaultClient`, sisi desktop)**: `credential_id`
TETAP tidak dikenal backend (server urus secret-nya sendiri per host/
identity `id`, bukan per `credential_id` terpisah) — jadi
`RemoteVaultClient` internal MENYAMAKAN `credential_id` dengan `id`
resource pemiliknya (host atau identity) SETIAP KALI membangun
`terminus_core::HostProfile`/`Identity` dari respons backend (di
`list_all_profiles`/`get`/dst) — nilai ini SELALU bisa direkonstruksi
ulang kapan pun (tidak perlu disimpan terpisah) karena `id` SELALU ada
di tiap respons.

Konsekuensinya buat urutan panggilan `store_secret` SEBELUM
`save_profile`/`save_identity` yang sudah tertanam di `state.rs`
(TIDAK DIUBAH, lihat bagian 2.1):
- `store_secret(credential_id, plaintext)` di `RemoteVaultClient`:
  COBA `PUT /hosts/:credential_id/secret` LANGSUNG dulu. Kalau 404
  (resource itu BELUM ada di server — kasus create baru, credential_id
  di sini masih murni UUID client-generated yang belum dikenal server
  sama sekali) -> BUFFER plaintext-nya di memori
  (`Arc<TokioMutex<HashMap<Uuid, Vec<u8>>>>`), TIDAK error ke caller.
  Kalau sebelumnya gagal di `/hosts/...` (404) DAN ini ternyata
  identity (bukan host) -> `store_secret` TIDAK BISA tahu duluan mana
  yang benar, makanya buffer-lah pendekatan yang dipilih (bukan coba
  `/identities/...` juga) — buffer BEKERJA UNTUK KEDUANYA lewat
  `save_profile`/`save_identity` di langkah berikutnya (lihat bawah).
- `save_profile(profile)`: `POST /hosts` (kirim `id`, lihat Perbaikan
  1) -> SETELAH sukses, cek buffer punya entry buat `credential_id`
  profil ini -> kalau ADA, `PUT /hosts/:id/secret` pakai plaintext dari
  buffer, lalu HAPUS dari buffer.
- `save_identity(identity)`: cek buffer buat `identity.credential_id`
  LEBIH DULU (SEBELUM `POST /identities`, beda dari host) -> WAJIB ADA
  (Identity tanpa password ditolak backend) -> `POST /identities` kirim
  `id` + `password` dari buffer LANGSUNG DALAM SATU REQUEST (backend
  tidak punya cara lain) -> hapus dari buffer.
- Path UPDATE (host/identity yang SUDAH ADA di server, `credential_id`
  == `id` yang server SUDAH kenal): `store_secret` yang PUT langsung
  sukses (bukan 404) -> SELESAI di situ, `save_profile`/`save_identity`
  yang menyusul TIDAK PERLU mengecek buffer (kosong buat credential_id
  ini).

### 2.7 `check_host_key`/`trust_host_key` DIKELUARKAN dari `VaultBackend` (ditemukan waktu baca `host_key_store.rs`)

`AppHostKeyStore` (`crates/app/src/host_key_store.rs`) menjembatani
`VaultStore::check_host_key`/`trust_host_key` ke trait
`terminus_ssh_engine::HostKeyStore` — trait itu **SINKRON MURNI**
(`fn lookup(&self, ...) -> Option<String>`, TIDAK ADA `async`, dipanggil
dari dalam SSH handshake `ssh-engine`). Memaksa ini lewat
`VaultBackend` yang `async` butuh ubah `ssh-engine` juga — DI LUAR
SCOPE pekerjaan ini.

**Keputusan**: `check_host_key`/`trust_host_key` TETAP di `VaultStore`
SAJA (bukan bagian `VaultBackend`), TIDAK ADA di `RemoteVaultClient`.
`AppHostKeyStore` diubah pegang `Option<Arc<std::sync::Mutex<VaultStore>>>`
(`None` di mode Self-hosted) — `VaultBackend` dapat 1 method tambahan
`local_store(&self) -> Option<Arc<std::sync::Mutex<VaultStore>>>`
(`Some` cuma buat varian `Local`) buat `main.rs` ambil ini waktu
bikin `AppHostKeyStore`.

**Konsekuensi yang SENGAJA diterima (didokumentasikan, bukan
disembunyikan)**: mode Self-hosted BELUM PUNYA persistensi
known_hosts sama sekali — tiap sesi SSH baru akan tanya ulang trust
host key (bukan lubang keamanan — tetap TOFU/trust-on-first-use yang
benar per sesi, cuma tidak "diingat" lintas restart app). Perbaikan
(bikin `known_hosts.db` kecil TERPISAH yang always-local di kedua
mode) DITUNDA ke milestone lain yang lebih kecil — supaya tidak
menyentuh skema `vault.db` Local yang sudah stabil di tengah pekerjaan
Self-hosted ini.

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
2. ✅ **`RemoteVaultClient` + `VaultBackend` enum** (`crates/vault/src/
   remote.rs` + enum baru di `lib.rs`) — 14 method CRUD (bukan 16,
   `check_host_key`/`trust_host_key` dikeluarkan, lihat 2.7) async
   native (retry-on-401 refresh transparan, buffer-secret-sebelum-
   resource-ada per 2.6). **Ditemukan & diperbaiki 3 masalah desain
   nyata SEBELUM/SELAMA nulis kode ini** (bukan sesudah): (a)
   `reqwest::blocking` panik di `spawn_blocking` -> `VaultBackend` jadi
   `async fn`, bagian 2.1; (b) ID client-authoritative vs REST
   server-authoritative -> backend nerima `id` opsional + buffer secret
   di `RemoteVaultClient`, bagian 2.6; (c) `check_host_key`/
   `trust_host_key` butuh trait SINKRON dari `ssh-engine`, tidak bisa
   ikut `VaultBackend` yang async -> dikeluarkan + `local_store()`
   helper, bagian 2.7. **Belum**: `state.rs` MASIH pakai `VaultStore`
   langsung (field `vault: Arc<Mutex<VaultStore>>` belum diganti) —
   SENGAJA displit jadi milestone terpisah (2b) karena volume perubahan
   di file 4300+ baris itu sendiri besar & berisiko, checkpoint di sini
   dulu. Diverifikasi: `cargo build -p terminus-vault` bersih TANPA
   warning (compile standalone, belum disentuh dari `state.rs` sama
   sekali), `cargo build --workspace` bersih, `cargo test -p
   terminus-app` 13/13 tetap hijau (belum ada test baru buat
   `RemoteVaultClient` sendiri — butuh mock HTTP server, DITUNDA ke
   milestone 2b bareng wiring `state.rs`, supaya test-nya sekalian
   nguji alur create-lalu-secret yang sebenarnya lewat `VaultBackend`,
   bukan `RemoteVaultClient` terisolasi).
3. ✅ **Milestone 2b — wiring `state.rs`** (~50 titik pemakaian vault
   di file 4300+ baris, dipetakan penuh sebelum eksekusi). **Temuan
   yang mengubah rencana awal**: sebagian titik baca (klik kartu host,
   buka grup, pencarian) SENGAJA ditulis SINKRON di `state.rs` asli,
   dan SATU test yang sudah ada (`alur_hosts_lengkap_...`) secara
   eksplisit menegaskan `on_identity_picked` "Sinkron (bukan
   tokio::spawn...)" — memaksa SEMUA titik itu jadi `async` akan
   mengubah kontrak yang sudah dites tanpa alasan kuat. **Solusi**:
   cache in-memory (`profiles_cache`/`groups_cache`/`identities_cache`
   di `AppState`), di-refresh (`refresh_vault_cache`, fungsi baru) dari
   `VaultBackend` (async) SETELAH SETIAP mutasi sukses, SEBELUM
   `refresh_hosts_model` (yang sekarang baca cache ini, TETAP SINKRON,
   signature TIDAK berubah) dipanggil. Titik baca cepat lain
   (`on_group_opened`/`on_search_requested`/`on_host_selected`, lookup
   `existing` di awal `on_host_save_requested`/`on_host_duplicate_
   requested`) tinggal ganti sumber baca ke cache, TETAP SINKRON, TIDAK
   ADA test yang perlu diubah kontraknya. **Satu-satunya titik yang
   genuinely tidak bisa dihindari jadi async**: `on_identity_picked`
   (butuh `read_secret`, password TIDAK ikut di-cache) — test terkait
   diperbarui pakai `wait_until`. `on_host_connect_requested`/
   `on_sftp_connect_requested` direstrukturisasi serupa (lookup profil
   dari cache, `read_secret` dipindah ke dalam task async). 3 fungsi
   helper Export/Import (`import_parsed_hosts_into_vault`/`export_
   hosts_from_vault`/`build_json_backup`) jadi `async fn`, closure
   pemanggilnya dipisah jadi 2 tahap (baca/tulis vault async dulu, baru
   dialog file native + tulis file tetap `spawn_blocking`, TIDAK
   menyentuh vault lagi). `check_host_key`/`trust_host_key` (lewat
   `AppHostKeyStore`) TETAP Local-only via `state.vault.local_store()
   .expect(...)` — Milestone 2b BELUM ada cabang startup Self-hosted
   (itu Milestone 3), `.expect()` dipakai sementara di titik itu &
   `is_first_run`/`initialize`/`unlock`. Diverifikasi: `cargo build
   --workspace` bersih TANPA warning, `cargo test -p terminus-app`
   **13/13 lulus** (termasuk test end-to-end besar: create/edit/
   duplicate/delete host & grup, drill-down, search, identity create/
   pick/delete, unlock salah/benar — tidak ada regresi).
4. ⏳ **UI `VaultDialog` tab Local/Self-hosted** + wiring
   login/register/refresh-token-silent-login di `main.rs`/`state.rs` +
   adaptasi `AppHostKeyStore` jadi `Option<Arc<Mutex<VaultStore>>>`.
5. ⏳ **Resolusi vault (bagian 4)**: auto-create/auto-select/pilih
   vault sesudah login, simpan `vault_id` ke `app_config.json`.
6. ⏳ **Verifikasi end-to-end manual**: jalankan `backend/` lewat
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
