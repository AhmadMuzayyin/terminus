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
  non-GPL. Cek ulang NOTICE.md di versi yang di-pin (Milestone 4).

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

Layar terminal TIDAK memakai kelas Termux langsung. Ada antarmuka kecil
`TerminalSurface` (tulis byte dari server ke tampilan, terima input user,
ukuran kolom×baris berubah, ambil teks seleksi, zoom font). Implementasi
v1: `TermuxTerminalSurface`. Kalau nanti pindah ke libghostty, cukup
tambah implementasi baru.

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
- **Instrumentasi (emulator)**: alur login & layar utama.
- **E2E ke backend sungguhan**: backend TERPISAH ber-database
  `terminus_e2e` (JANGAN DB berisi akun sungguhan — suite auth backend
  menghapus user), mis. `PORT=4100`. Dari emulator Android, host PC =
  `http://10.0.2.2:4100`. SSH uji ke server/container SSH lokal.

## 9. Urutan Pengerjaan (Milestone)

Dikerjakan SATU-SATU, tiap milestone diverifikasi (build + test + dicoba
di emulator/HP) sebelum lanjut; user yang commit.

1. ⏳ **Kerangka proyek** — Gradle wrapper, Compose, Material 3, bottom
   navigation dengan 4 layar kosong, `applicationId` placeholder, lisensi.
   Verifikasi: `./gradlew assembleDebug` + jalan di emulator.
2. ⏳ **Login & sesi** — client API (auth, vault), token store Keystore,
   refresh-on-401, resolusi vault, login otomatis, daftar admin pertama,
   logout. Unit test MockWebServer.
3. ⏳ **Hosts & Identities** — CRUD grup/host/identity + password,
   pencarian, drill-down grup.
4. ⏳ **Terminal SSH** — sshj + known_hosts, `TerminalSurface` + Termux,
   tombol ekstra, multi-sesi, foreground service, salin/tempel, resize.
   Pin versi Termux + cek NOTICE.md.
5. ⏳ **SFTP** — jelajah, unduh/unggah via SAF, buat folder/ganti
   nama/hapus.
6. ⏳ **Akun & pengaturan** — profile (nama/email/password), kunci app
   biometrik, ukuran font & tema terminal.
7. ⏳ **Verifikasi end-to-end** ke backend lokal + build APK rilis
   (tanda tangan debug/rilis, package name masih placeholder).

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
