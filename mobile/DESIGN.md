# Terminus Mobile (Android) — Desain Arsitektur

> Dokumen ini SUMBER KEBENARAN buat pekerjaan `mobile/`, sama seperti
> `backend/DESIGN.md` buat backend. WAJIB dibaca ulang tiap mau lanjut
> kerja di sini (termasuk lintas sesi) SEBELUM nulis kode. Update dokumen
> ini kalau ada keputusan yang berubah. Dokumen desain TIDAK BOLEH
> ditaruh di `docs/` — folder itu khusus dokumentasi penggunaan
> (Terminus Local, Self-hosted, dan Mobile).

## 1. Apa ini

Aplikasi Android native yang jadi **client server Terminus Self-hosted**
(`backend/`). Data host/grup/identity/password SAMA dengan yang dilihat
desktop app mode Self-hosted — dibaca/ditulis langsung ke server lewat
HTTP (thin-client, bukan sinkronisasi/offline-first, sama prinsip dengan
desktop, lihat `backend/DESIGN.md` bagian 1).

Yang disimpan di HP cuma: token login (terenkripsi), URL server + id
vault, host key SSH yang sudah dipercaya (`known_hosts`), dan pengaturan
app. TIDAK ADA salinan host/password di HP.

## 2. Keputusan yang Sudah Disepakati (diskusi 2026-09-30)

| Topik | Keputusan | Alasan singkat |
|---|---|---|
| Platform | **Android dulu**, iOS nanti (di luar scope) | Sesuai roadmap; iOS butuh Mac + akun Apple |
| Stack | **Kotlin native** + Jetpack Compose | Toolchain Android paling standar |
| Terminal | **Library Termux** (`terminal-emulator` + `terminal-view`) | Paling matang di Android: IME, seleksi sentuh, zoom |
| Masa depan terminal | Dibungkus antarmuka sendiri (bagian 4.4) supaya bisa ditukar ke **libghostty-android** kalau sudah stabil (sekarang masih alpha) | Fitur ala kitty (keyboard/graphics protocol) tanpa bongkar layar lain |
| Mode | **Self-hosted saja** (tidak ada vault lokal) | Paling berguna di HP; separuh pekerjaan |
| Fitur v1 | **Semua**: Hosts+grup, terminal SSH, Identities, SFTP, Profile akun | — |
| Navigasi | **Bottom navigation**: Hosts, SFTP, Identities, Akun | Standar Android, terjangkau jempol |
| Kunci app | **Opsional** di Pengaturan (default MATI): biometrik / kunci layar HP | Pilihan user |
| Min Android | **Android 10 (API 29)** | Scoped storage modern, kode lebih sederhana |
| Package name | Placeholder **`com.example.terminus`** — diganti user sebelum rilis | Cuma di `applicationId` (bagian 4), terpisah dari namespace kode |
| Auth SSH | **Password saja** di v1 | Server baru bisa simpan password teks (sama dengan desktop Self-hosted) |

## 3. Tech Stack & Alasan

- **Kotlin + Jetpack Compose** (Material 3), **Coroutines/Flow** buat
  semua I/O. Satu modul Gradle `app` dulu (bukan multi-modul) — pecah
  modul baru kalau memang ada alasan nyata, bukan di awal.
- **Terminal**: `com.termux.termux-app:terminal-view` (+ `terminal-emulator`
  ikut sebagai dependency) via JitPack, **versi DI-PIN** (tidak pakai
  versi dinamis). `TerminalView` itu View klasik → ditanam di Compose
  lewat `AndroidView`.
- **SSH & SFTP**: **sshj** (+ BouncyCastle yang dibutuhkannya di
  Android). Satu koneksi SSH bisa buka shell (terminal) dan subsistem
  SFTP.
- **HTTP**: **OkHttp** + **kotlinx.serialization** (JSON). Tanpa Retrofit
  — endpoint sedikit, client tipis ditulis tangan lebih mudah dikontrol
  (terutama refresh-on-401, bagian 5).
- **Penyimpanan lokal**: **DataStore** (config, pengaturan, known_hosts)
  + **Android Keystore** (kunci AES-GCM non-exportable) buat enkripsi
  refresh token.
- **Kunci app**: **androidx.biometric** (`BIOMETRIC_STRONG or
  DEVICE_CREDENTIAL`).
- **DI**: manual (satu objek `AppContainer`), tanpa Hilt/Koin — app kecil,
  hindari abstraksi berlebih (sama gaya dengan backend: Express, bukan
  NestJS).
- **Lisensi**: aplikasi ini **GPL-3.0-or-later**, sama dengan repo.
  Catatan: `terminal-emulator`/`terminal-view` Apache 2.0, TAPI versi
  baru `terminal-view` memuat renderer glyph turunan kitty berlisensi
  GPLv3 (lihat `terminal-view/NOTICE.md` di termux-app) — cocok dengan
  GPLv3 kita, tapi berarti APK mobile TIDAK BOLEH dirilis dengan lisensi
  non-GPL. **Dicek di Milestone 4**: versi yang di-pin (`v0.118.3`) belum
  punya NOTICE.md itu — murni Apache 2.0. Cek ulang tiap naik versi.
- **Termux dipakai sebagian di-vendor** (Milestone 4): `TerminalSession`
  Termux itu `final` & selalu menjalankan proses LOKAL lewat JNI, dan
  `TerminalView` cuma menerima tipe itu. Jadi: `terminal-emulator` dipakai
  apa adanya (JitPack), sedangkan kode `terminal-view` DISALIN ke
  `app/src/main/java/com/termux/view/` dengan satu perubahan — tipe
  `TerminalSession` diganti `TerminalViewSession` (kelas kita). Detail &
  atribusi: `mobile/THIRD_PARTY_NOTICES.md`. Naik versi Termux = salin
  ulang folder itu + ulangi penggantian yang sama.

## 4. Struktur Folder & Lapisan

```
mobile/
  DESIGN.md                ← dokumen ini
  settings.gradle.kts, build.gradle.kts, gradle/…
  app/
    build.gradle.kts       ← applicationId = "com.example.terminus" (SATU-SATUNYA tempat)
    src/main/java/…/terminus/
      MainActivity.kt
      AppContainer.kt      ← wiring manual semua dependency
      api/                 ← client HTTP backend (bagian 5)
      auth/                ← sesi login, token store (Keystore), resolusi vault
      data/                ← model domain + repository (host/grup/identity)
      ssh/                 ← koneksi sshj, known_hosts, SessionManager, SFTP
      terminal/            ← TerminalSurface (antarmuka) + implementasi Termux
      ui/                  ← layar Compose per fitur (login, hosts, terminal, sftp, identities, account)
    src/test/…             ← unit test JVM (MockWebServer buat api/)
    src/androidTest/…      ← test instrumentasi (emulator)
```

Aturan arah dependency: `ui` → `data`/`ssh`/`auth` → `api`. `api` tidak
tahu apa pun soal Android UI (gampang dites di JVM).

### 4.4 Antarmuka terminal yang bisa ditukar

Layar terminal TIDAK memakai kelas Termux langsung. Wujud akhirnya
(Milestone 4, `terminal/TerminalEngine.kt`):
- `ssh/ShellChannel` — aliran byte shell (output, write, resize, close).
  `ssh/` tidak tahu apa pun soal Termux/layar.
- `EmulatorSession` — isi layar + riwayat satu sesi, diberi makan
  `ShellChannel`; hidup lebih lama dari view (pindah tab/rotasi/sambung
  ulang tidak menghapus isinya).
- `TerminalSurface` — view yang menampilkan satu `EmulatorSession`
  (tombol ekstra, tempel, keyboard).
- `TerminalEngine` — pembuat keduanya. Implementasi v1: `TermuxEngine`
  (`TermuxSession` + `TermuxTerminalSurface`). Pindah ke libghostty =
  engine baru; `ssh/`, `TerminalSessions`, & layar lain tetap.

## 5. Kontrak API Backend yang Dipakai

TIDAK ADA perubahan backend untuk v1. Semua di bawah `/api/v1`:

- **Auth**: `POST /auth/register` (`email`, `password`, `fullName` —
  cuma sukses waktu server belum punya user), `POST /auth/login`,
  `POST /auth/refresh` (rotasi: token lama langsung dicabut),
  `POST /auth/logout`, `GET/PATCH /auth/me`, `PUT /auth/me/password`.
- **Vault**: `GET /vaults`, `POST /vaults`.
- **Grup**: `GET/POST /vaults/:v/groups`, `GET/PUT/DELETE …/groups/:id`
  (hapus grup ikut menghapus host di dalamnya — konfirmasi di UI).
- **Host**: `GET/POST …/hosts`, `GET/PUT/DELETE …/hosts/:id`,
  `GET/PUT …/hosts/:id/secret` (password). Username host boleh kosong.
- **Identity**: `GET/POST …/identities` (POST wajib `password`),
  `GET/PUT/DELETE …/identities/:id` (PUT `password` opsional = tidak
  diubah), `GET …/identities/:id/secret`.

Aturan client (SAMA dengan desktop, sudah teruji di sana):
- **401 → refresh sekali lalu ulangi request**, transparan buat layar.
  Refresh ditolak (401) → sesi berakhir: hapus token, kembali ke Login.
  Kegagalan jaringan/5xx waktu refresh → token TIDAK dibuang.
- **Refresh token hasil rotasi WAJIB langsung dipersist**; setelah user
  menekan Logout, token hasil rotasi TIDAK boleh dipersist lagi (race
  dengan request di background).
- **Password saat ini salah di endpoint profil = 403** (bukan 401) —
  tampilkan pesannya, JANGAN diperlakukan sebagai sesi kedaluwarsa.
- **Beda dari desktop**: mobile BOLEH server-authoritative murni (biar
  server yang generate id; host baru dibuat dulu, baru `PUT /secret`).
  Kerumitan buffer password di desktop tidak diperlukan di sini.
- **Resolusi vault** sesudah login, SAMA dengan desktop: pakai `vault_id`
  tersimpan kalau user masih anggotanya; kalau tidak, vault pertama; 0
  vault → buat "My Vault". Pemilih vault (>1 vault) DI LUAR scope v1.

## 6. Penyimpanan Lokal & Keamanan

- **Refresh token**: dienkripsi AES-GCM dengan kunci Android Keystore
  (tidak bisa diekspor dari HP), disimpan di DataStore. Access token
  cuma di memori.
- **Logout**: tandai logout → hapus token tersimpan (sinkron) → `POST
  /auth/logout` best-effort → tutup semua sesi SSH/SFTP → layar Login
  (URL server tetap terisi).
- **known_hosts**: disimpan di HP (DataStore), per `host:port`. Host baru
  → dialog sidik jari (fingerprint SHA256) "Percayai?". Host key BERUBAH →
  koneksi DIBLOKIR + peringatan (kemungkinan MITM), user bisa hapus entri
  lama secara eksplisit. Lebih ketat dari desktop mode Self-hosted (di
  sana belum ada known_hosts).
- **Kunci app (opsional)**: kalau aktif, minta biometrik/kunci layar
  waktu app dibuka & setelah 5 menit di background.
- **Tidak ada** password host yang disimpan di HP; diambil dari server
  tiap mau connect, dibuang dari memori setelah autentikasi.
- `android:allowBackup="false"` — token & known_hosts tidak ikut backup
  cloud Android.
- HTTP polos (`http://`) diizinkan (server self-hosted di LAN), TAPI UI
  menampilkan peringatan "koneksi tidak terenkripsi" di layar Login.

## 7. Alur UI

**Login** (tampil kalau belum login): Server URL, Email, Password,
tombol Masuk; link "Server baru? Daftar admin pertama" → form Daftar
(Nama Lengkap, Email, Password, Konfirmasi). Start app dengan token
tersimpan → masuk otomatis ("Masuk otomatis…").

**Bottom navigation** (setelah login):
1. **Hosts** — daftar grup + host tanpa grup, masuk grup (drill-down),
   pencarian, FAB tambah host/grup. Ketuk host → connect terminal. Tahan
   (long-press) → edit/duplikat/hapus. Form host: label, host, port,
   username, grup, password (atau pilih Identity → isi username+password).
2. **SFTP** — pilih host → jelajah folder server; unduh file ke HP &
   unggah dari HP lewat pemilih file sistem (Storage Access Framework,
   tanpa izin storage luas); buat folder, ganti nama, hapus.
3. **Identities** — daftar, tambah (label, username, password), edit,
   hapus.
4. **Akun** — nama & email; Profile (ubah nama, email [wajib password
   saat ini], password [semua perangkat lain logout]); Pengaturan (kunci
   app, ukuran font terminal, tema terminal); Logout (dengan konfirmasi).

**Terminal** (layar penuh di atas bottom nav, dibuka dari Hosts):
- Beberapa sesi sekaligus; pengalih sesi di bar atas (label host,
  tombol tutup).
- Host belum punya password → minta password dulu (opsi simpan ke
  server), baru connect.
- Baris tombol ekstra di atas keyboard: `Esc`, `Ctrl`, `Alt`, `Tab`,
  `←` `↑` `↓` `→`, `-`, `/`, `|`, `~` (Ctrl/Alt bersifat "tempel" — tekan
  sekali lalu huruf).
- Seleksi teks dengan tahan-geser → Salin; menu Tempel. Pinch = ukuran
  font. Scroll riwayat dengan geser.
- Ukuran terminal ikut layar/rotasi → `window-change` ke server.
- **Sesi tetap hidup waktu app ke background** lewat *foreground
  service* dengan notifikasi "N sesi SSH aktif" (tanpa ini Android bisa
  mematikan koneksi). Menutup sesi terakhir menghentikan service.

## 8. Pengujian

- **Unit test JVM**: client API dengan **MockWebServer** (refresh-on-401,
  rotasi token, 403 profil, resolusi vault), parser/format kecil.
- **Instrumentasi (emulator)**: alur login & layar utama —
  `app/src/androidTest/.../LoginE2ETest.kt` (login benar -> daftar host,
  password salah -> pesan). Server & akun lewat argumen instrumentasi,
  cara menjalankan di kepala file itu.
- **E2E ke backend sungguhan**: backend TERPISAH ber-database
  `terminus_e2e` (JANGAN DB berisi akun sungguhan — suite auth backend
  menghapus user), mis. `PORT=4100`. Dari emulator Android, host PC =
  `http://10.0.2.2:4100`. SSH uji ke server/container SSH lokal.

## 9. Urutan Pengerjaan (Milestone)

Dikerjakan SATU-SATU, tiap milestone diverifikasi (build + test + dicoba
di emulator/HP) sebelum lanjut; user yang commit.

1. ✅ **Kerangka proyek** — Gradle wrapper, Compose, Material 3, bottom
   navigation dengan 4 layar kosong, `applicationId` placeholder, lisensi.
   Verifikasi: `./gradlew assembleDebug` + jalan di emulator.
   **Selesai** — versi: Gradle 9.8.0, AGP 9.4.1 (Kotlin bawaan AGP, tanpa
   plugin `kotlin-android`), Kotlin compose plugin 2.4.20, Compose BOM
   2026.09.00. Temuan yang mengubah rencana:
   - `compileSdk = 37` (bukan 36): `core-ktx` 1.19.1 mensyaratkannya;
     `targetSdk` tetap 36 (perilaku runtime tidak berubah).
   - Mesin dev cuma punya JRE (tanpa `javac`) → JDK 21 buat compile
     diunduh otomatis lewat foojay toolchain resolver
     (`settings.gradle.kts` + `java.toolchain` 21), tanpa sudo.
   - Namespace kode `org.terminus.mobile`; `applicationId`
     `com.example.terminus` (placeholder).
   - Status bar dipaksa ikon terang (`SystemBarStyle.dark`) — app selalu
     gelap; default mengikuti tema sistem & tidak terbaca di HP bertema
     terang.
   - Warna = token desktop (`ui/tokens.slint`), ikon garis = path yang
     sama dengan `ui/components/icons.slint`.
   Diverifikasi: build debug sukses, unit test 1/1, dijalankan di emulator
   AVD `A16` (Android 16) — 4 tab berpindah, tanpa crash.
2. ✅ **Login & sesi** — client API (auth, vault), token store Keystore,
   refresh-on-401, resolusi vault, login otomatis, daftar admin pertama,
   logout. Unit test MockWebServer.
   **Selesai** — `api/` (murni JVM: `HttpClient`, `AuthApi`, `ApiSession`
   dengan refresh-on-401 ber-`Mutex` supaya beberapa request 401
   bersamaan cuma refresh SEKALI), `auth/` (`KeystoreTokenStore`
   AES-GCM, `DataStoreConfigStore`, `SessionManager` + `AuthState`),
   `TerminusApplication`/`AppContainer`, layar Login/Daftar, splash
   "Masuk otomatis…", tab Akun (identitas + Logout berkonfirmasi).
   `network_security_config` mengizinkan `http://` (peringatan tampil di
   form). Unit test 17/17 (ApiSession 6, SessionManager 10, Tab 1).
   Diuji di emulator lawan backend TERPISAH (port 4100, DB
   `terminus_e2e`, dari emulator `http://10.0.2.2:4100`): daftar admin
   pertama (+ "My Vault" otomatis), login otomatis setelah force-stop
   (token rotasi tersimpan: 1 dicabut, 1 aktif), logout (token dicabut
   server, URL tetap, buka ulang minta login), password salah
   ("Email atau password salah"), login manual.
   **Catatan uji**: di emulator yang punya Google Autofill aktif, isian
   `adb shell input text` tercampur saran autofill — matikan sementara
   (`settings put secure autofill_service null`) lalu KEMBALIKAN.
3. ✅ **Hosts & Identities** — CRUD grup/host/identity + password,
   pencarian, drill-down grup.
   **Selesai** — `api/VaultApi` + `VaultModels`, `data/` (`VaultRepository`
   satu sumber isi vault per sesi login; `Forms` & `HostListing` fungsi
   murni), `ui/hosts` (daftar + drill-down, pencarian, FAB, menu ⋮ /
   tahan lama, form host), `ui/identities`, `ui/common`. Keputusan:
   - Grup **satu tingkat** (sama dengan desktop — `parentId` tidak dipakai).
   - Tiap perubahan -> kirim ke server -> muat ulang daftar (tanpa cache
     lokal). Mutasi `NonCancellable` supaya "buat host -> kirim password"
     tidak terpotong pindah tab.
   - Host baru tersimpan tapi password gagal -> form pindah ke mode edit
     host itu (`HostPasswordNotSaved`), mencegah host dobel.
   - `groupId: null` dikirim EKSPLISIT (encoder `explicitNulls = true`);
     `password` identity yang tidak diubah DIBUANG dari JSON (backend
     menolak `null`). Field tags/kind/terminalTheme tidak pernah dikirim.
   - Label host kosong = pakai host/IP. Duplikat ikut menyalin password.
   - Ketuk host = snackbar "belum tersedia" sampai Milestone 4.
   Unit test 34/34 (baru: Forms 6, HostListing 4, VaultRepository 7).
   Diuji di emulator lawan backend 4100/`terminus_e2e`: tambah identity,
   grup, host di dalam grup diisi dari identity (password di server
   cocok), duplikat (password ikut), edit + pindah ke Tanpa grup (server:
   `groupId` null, port 2222), pencarian multi-kata, Back keluar grup,
   hapus grup (host di dalamnya ikut terhapus).
4. ✅ **Terminal SSH** — sshj + known_hosts, `TerminalSurface` + Termux,
   tombol ekstra, multi-sesi, foreground service, salin/tempel, resize.
   Pin versi Termux + cek NOTICE.md.
   **Selesai** — versi: Termux `v0.118.3` (Apache 2.0, tanpa NOTICE.md
   GPL), sshj 0.41.1, BouncyCastle 1.84. Keputusan & temuan:
   - `terminal-view` di-vendor (lihat bagian 3 & 4.4).
   - **Host key dua tahap**: host baru -> koneksi pertama ditolak di
     verifier & sidik jarinya dilaporkan -> dialog "Percayai?" -> simpan
     -> connect ulang. Menunggu jawaban user DI DALAM verifier sshj tidak
     bisa (thread transport dengan timeout key exchange). Password
     disimpan di memori HANYA selama dialog itu terbuka.
   - known_hosts per `host`/`[host]:port`, format sidik jari = OpenSSH
     (`SHA256:…`, diuji sama persis dengan `ssh-keygen -lf`). Kunci
     berubah (termasuk jenis kunci berbeda) = DIBLOKIR; satu-satunya
     jalan "Hapus kunci lama" (berkonfirmasi) lalu periksa kunci baru.
   - Android membawa provider "BC" versi pangkas -> diganti BouncyCastle
     lengkap di `TerminusApplication` (tanpa ini Ed25519 gagal).
   - Sesi seumur PROSES (`TerminalSessions` di `AppContainer`), bukan
     layar. Foreground service tipe `specialUse` (dataSync dibatasi 6 jam
     di Android 15) cuma memegang notifikasi; berhenti sendiri di 0 sesi.
     Izin notifikasi (Android 13+) ditanya waktu connect pertama.
   - Username/password kosong atau autentikasi gagal -> dialog kredensial
     (opsi "Simpan ke server"); gagal login mengisi ulang username yang
     barusan dicoba.
   - Sambung ulang memakai tab & layar yang sama (riwayat tetap);
     keepalive 30 detik; `configChanges` di Activity -> rotasi cukup
     resize, tidak membongkar view.
   - Landscape + keyboard menempel: bar sesi disembunyikan & baris tombol
     ekstra dirapatkan (tanpanya terminal tinggal 0–4 baris).
   - Ukuran font (pinch) belum disimpan permanen — Milestone 6.
   Unit test 38/38 (baru: HostKeys 4). Diuji di emulator lawan backend
   4100/`terminus_e2e` + container `linuxserver/openssh-server` (dari
   emulator `10.0.2.2:2222`): dialog sidik jari (cocok dengan
   `ssh-keyscan`), shell & input, `stty size` ikut layar/rotasi (51 -> 108
   kolom), CTRL tempel (^C), panah riwayat, seleksi -> Salin -> Tempel,
   dua sesi + pengalih, username/password ditanya + password salah + simpan
   ke server, `exit` -> Sambung ulang (riwayat tetap), container dibuat
   ulang -> "Koneksi terputus" lalu HOST KEY BERUBAH diblokir -> hapus
   kunci lama -> kunci baru dipercaya, logout -> service berhenti & 0
   koneksi SSH tersisa di server.
5. ✅ **SFTP** — jelajah, unduh/unggah via SAF, buat folder/ganti
   nama/hapus.
   **Selesai** — `ssh/SftpChannel` (sshj SFTP), `sftp/SftpManager` +
   `RemotePaths` (fungsi murni), `ui/sftp/SftpScreen`. Keputusan:
   - `SshConnector` punya SATU jalur connect + known_hosts + login untuk
     shell & SFTP, dan satu instance dipakai bersama — aturan host key
     identik. Tampilan dialog/peringatan host key juga dipakai bersama
     (`ui/common/ConnectionUi.kt`).
   - `ConnectFlow` jadi generik (`onReady(host, username, password)`):
     dialog kredensial & "Simpan ke server" sama untuk terminal & SFTP.
   - SATU koneksi SFTP seumur proses (bukan layar): pindah tab tidak
     memutus koneksi/transfer; ikut dihitung foreground service
     ("1 sesi SSH + SFTP aktif"); logout ikut memutusnya. Sambung ulang
     ke host yang sama kembali ke folder terakhir.
   - Satu transfer sekaligus (tanpa antrean) dengan progres & Batal.
     Gagal/batal -> file setengah jadi DIHAPUS (di HP untuk unduhan, di
     server untuk unggahan).
   - Unduh = SAF "Simpan sebagai" (`CreateDocument`), unggah = SAF
     `OpenDocument` + konfirmasi "Timpa?" kalau nama sudah ada. Tanpa izin
     storage sama sekali.
   - Hapus folder = rekursif, dengan peringatan "BESERTA SELURUH ISINYA";
     symlink tidak pernah diikuti saat menghapus (yang dihapus link-nya).
     Mengetuk symlink mengikuti tujuannya (folder dibuka, file diunduh).
   - Bar progres memakai track netral (`ProgressBar`): track bawaan
     Material = `secondaryContainer` = hijau di tema ini, bar 6% terlihat
     hampir penuh.
   Unit test 42/42 (baru: RemotePaths 4). Diuji di emulator lawan backend
   4100/`terminus_e2e` + container SSH: host key berubah diblokir juga di
   SFTP -> hapus -> percayai kunci baru, jelajah + symlink ke folder, unduh
   (5 MB, SHA-256 sama dengan server), buat folder, ganti nama, unggah
   (isi di server cocok), konfirmasi timpa, hapus folder berisi, unduh
   400 MB -> progres -> Batal (file parsial di HP terhapus), notifikasi
   "SFTP aktif" / "1 sesi SSH + SFTP aktif", koneksi tetap hidup waktu app
   di background, logout -> 0 koneksi tersisa di server.
6. ✅ **Akun & pengaturan** — profile (nama/email/password), kunci app
   biometrik, ukuran font & tema terminal.
   **Selesai** — `settings/Settings.kt` (DataStore `settings`),
   `terminal/TerminalThemes.kt`, `data/ProfileForms.kt`, `auth/AppLock.kt`,
   `ui/lock/DeviceAuth.kt`, `ui/account/{AccountScreen,ProfileScreen}.kt`.
   Keputusan & temuan:
   - Profil: aturan & pesan SALINAN `plan_account_update` desktop (cuma
     field berubah yang dikirim; ganti email wajib password saat ini).
     403 = pesan biasa (TIDAK logout). Ganti password: token baru dari
     server langsung dipasang & dipersist; perangkat lain logout.
   - Tema: 6 tema = SALINAN PERSIS `built_in_themes()` desktop
     (`crates/term-emulator/src/palette.rs`). Tema host dari server
     (`terminalTheme`, diatur di desktop) menang; kalau tidak dikenal ->
     tema default Pengaturan. Ganti tema default -> sesi terbuka ikut.
     Perintah `reset` di server mengembalikan warna Termux ke bawaan ->
     dideteksi (warna == skema bawaan persis) & tema dipasang ulang;
     warna yang sengaja diubah aplikasi server (OSC) tetap dihormati.
   - Ukuran font: pinch & slider disimpan permanen (slider menyimpan waktu
     dilepas). Berlaku untuk layar terminal yang dibuka berikutnya.
   - Kunci app: `androidx.biometric` 1.1.0 (rilis STABIL terakhir; 1.4.0
     masih alpha) + `MainActivity` jadi `FragmentActivity`. Biometrik kuat
     ATAU kunci layar (API 29 tidak mendukung kombinasi itu -> biometrik
     biasa ATAU kunci layar). Menyalakan & mematikan kunci wajib lolos
     autentikasi. Status "belum diketahui" selama pengaturan dibaca ->
     isi app tidak pernah sempat terlihat. Background dihitung dari
     onStop/onStart Activity (aman karena `configChanges`). Sesi SSH/SFTP
     tetap jalan selama terkunci; posisi layar (tab, grup) boleh hilang.
   - Slider & bar progres: track kosong dibuat netral (bawaan =
     `secondaryContainer` = hijau di tema ini, terlihat "penuh").
   Unit test 56/56 (baru: TerminalThemes 3, ProfileForms 5, AppLock 4,
   ApiSession +2). Diuji di emulator lawan backend 4100/`terminus_e2e`:
   tema Mono Amber + font 16 sp di terminal sungguhan, tema bertahan
   setelah `reset`, pengaturan tersimpan setelah force-stop, ganti email
   tanpa/salah password (pesan, tidak logout), ubah nama+email (server
   cocok), ganti password (token perangkat lain 401, app tetap masuk
   setelah force-stop), kunci app dengan PIN uji (aktifkan, buka ulang ->
   terkunci, batal -> layar kunci, buka dengan PIN, background singkat
   tidak mengunci, matikan wajib PIN). Background 5 menit dicakup unit
   test (jam palsu), tidak ditunggu sungguhan di emulator.
7. ✅ **Verifikasi end-to-end** ke backend lokal + build APK rilis
   (tanda tangan debug/rilis, package name masih placeholder).
   **Selesai** — build rilis (R8 + pengecilan resource + penandatanganan,
   lihat bagian 11) & test instrumentasi `LoginE2ETest`. Temuan:
   - R8 tanpa warning dengan aturan keep BouncyCastle & sshj
     (`app/proguard-rules.pro`). APK rilis 11 MB (debug 45 MB).
   - APK rilis diuji dari INSTALASI BERSIH lawan backend 4100 +
     container SSH: login (URL diketik), daftar host, SSH (sidik jari
     cocok `ssh-keyscan`, perintah jalan), SFTP (unduh, isi cocok),
     Identities, Profil — tanpa ClassNotFound/NoSuchMethod/
     Serialization error di logcat.
   - Test instrumentasi butuh `espresso-core` 3.7.0 eksplisit: versi
     lama bawaan `ui-test-junit4` memanggil `InputManager.getInstance`
     yang dihapus di Android 16 -> semua test gagal.
   - Orchestrator + `clearPackageData`: tiap test mulai dari data app
     bersih. 2/2 lulus, diulang 2x berturut-turut. SEKALI (run pertama
     setelah ganti dependency) teardown test kedua gagal "activity tetap
     PAUSED" — tidak muncul lagi di 3 run berikutnya; kalau berulang,
     curigai jendela sistem (autofill/izin) yang menutupi activity.
   - Tanpa argumen server, test dilewati (AssumptionViolated): task
     Gradle SUKSES, walau XML reporter AGP menulisnya di tag `<failure>`.

## 10. Sengaja DI LUAR SCOPE (jangan dikerjakan tanpa diminta)

- iOS.
- Mode Local (vault di HP tanpa server).
- Autentikasi SSH pakai private key (butuh perubahan backend dulu).
- Pemilih/penukar vault (user dengan >1 vault → vault pertama dipakai).
- Console serial (USB OTG), port forwarding, Telnet/Mosh.
- libghostty (baru dipertimbangkan setelah stabil — antarmuka 4.4 sudah
  menyiapkan jalannya).
- Import/Export XML/JSON (bisa lewat desktop, data servernya sama).
- Publikasi Play Store (package name final, ikon/branding final).

## 11. Build Rilis & Penandatanganan

- `./gradlew assembleRelease` -> `app/build/outputs/apk/release/app-release.apk`.
  R8 + pengecilan resource AKTIF; aturan keep di `app/proguard-rules.pro`
  (tiap aturan ada alasannya — jangan tambah `-keep class **` asal lolos).
  **Tiap kali dependency/aturan R8 berubah, APK rilis wajib diuji ulang
  penuh** (login, SSH, SFTP) — R8 cuma ketahuan merusak reflection saat
  runtime.
- **Kunci rilis** dibaca dari `mobile/keystore.properties`:
  ```
  storeFile=../terminus-release.jks   (relatif ke folder mobile/)
  storePassword=...
  keyAlias=terminus
  keyPassword=...
  ```
  File itu, `*.jks`, & `*.keystore` ada di `.gitignore` — JANGAN pernah
  di-commit. Keystore dibuat & disimpan SENDIRI oleh pemilik app (backup
  di tempat aman): kalau hilang, update app di Play Store tidak bisa
  dirilis lagi. Membuatnya: `keytool -genkeypair -v -keystore
  terminus-release.jks -alias terminus -keyalg RSA -keysize 4096 -validity 10000`.
- `keystore.properties` tidak ada -> APK rilis ditandatangani kunci
  DEBUG (bisa dipasang untuk uji, BUKAN untuk Play Store).
- Sebelum publikasi: ganti `applicationId` (placeholder
  `com.example.terminus`, satu-satunya tempat di `app/build.gradle.kts`),
  naikkan `versionCode`/`versionName`, ikon & branding final (bagian 10).
