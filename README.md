# Terminus

> SSH · SFTP · Console (serial) manager desktop native, lintas platform,
> ditulis 100% [Rust](https://www.rust-lang.org/) dengan GUI
> [Slint](https://slint.dev) — open-source, satu binary, tanpa
> Electron, tanpa runtime tambahan.

![License](https://img.shields.io/badge/lisensi-GPL--3.0--or--later-blue)
![Rust](https://img.shields.io/badge/rust-2021%20edition-orange)
![Status](https://img.shields.io/badge/status-aktif%20dikembangkan-green)

## Daftar Isi

- [Tentang Proyek](#tentang-proyek)
- [Fitur Utama](#fitur-utama)
- [Tumpukan Teknologi](#tumpukan-teknologi)
- [Struktur Proyek](#struktur-proyek)
- [Prasyarat & Instalasi](#prasyarat--instalasi)
- [Menjalankan Aplikasi](#menjalankan-aplikasi)
- [Menjalankan Test](#menjalankan-test)
- [Packaging & Rilis](#packaging--rilis)
- [Isu Dependency yang Sudah Diperbaiki](#isu-dependency-yang-sudah-diperbaiki)
- [Status & Roadmap](#status--roadmap)
- [Berkontribusi](#berkontribusi)
- [Lisensi](#lisensi)

## Tentang Proyek

Terminus adalah aplikasi manajemen koneksi remote (SSH, SFTP, dan
console serial) yang dibangun sebagai aplikasi desktop
native — bukan aplikasi web yang dibungkus (tidak ada Electron/Chromium
di dalamnya). Seluruh logic (kripto, transport SSH/SFTP/serial, parsing
terminal) ditulis di Rust dan dites langsung tanpa lapisan JavaScript,
sementara UI-nya dideklarasikan lewat [Slint](https://slint.dev) —
toolkit GUI native yang di-compile jadi kode Rust saat build.

Proyek ini open source di bawah lisensi **GPL-3.0-or-later** — silakan
di-fork, dipelajari, dan dikontribusikan. Lihat bagian
[Berkontribusi](#berkontribusi) di bawah kalau tertarik bantu
kembangkan.

## Fitur Utama

### 🔐 Manajemen Host & Vault Terenkripsi

- Simpan profil koneksi (label, host/IP, port, username, tags, grup)
  lengkap dengan kredensialnya, dienkripsi lokal pakai **Argon2id**
  (derive key dari master password) + **ChaCha20-Poly1305** (AEAD).
  Metadata tetap plaintext (biar cepat di-search), hanya kolom secret
  yang terenkripsi — tersimpan di SQLite lokal
  (`~/.config/terminus/vault.db`).
- Organisasi host lewat mode **List** (host tanpa grup) atau **Groups**
  (grid kartu grup, klik untuk drill-down) — satu host cuma pernah ada
  di salah satu, tidak pernah dobel.
- **Panel Host Details** (bukan modal popup) muncul permanen di kanan
  begitu satu host dipilih — edit label/host/port/username/password/
  tags/grup langsung di situ, tombol **Duplicate** (gandakan host +
  kredensialnya secara independen), reveal/hide password (👁), dan
  konfirmasi eksplisit sebelum menghapus.
- Pencarian global (kotak di TopBar) menembus semua halaman & semua
  grup — cocok di label, host/IP, username, `user@host`, atau tags.
- **Import dari [VanDyke SecureCRT](https://www.vandyke.com/products/securecrt/)**
  (`config.xml`) — folder bertingkat di-flatten jadi nama grup gabungan,
  dedup otomatis kalau diimpor ulang. Password **tidak pernah** ikut
  diimpor (terenkripsi proprietary VanDyke, secara sengaja tidak
  dibongkar) — host hasil import ditandai tag `imported`, tinggal isi
  ulang password-nya manual.

### 💻 Terminal SSH Interaktif

- Connect ke sebuah host membuka **tab terminal baru** (mirip browser)
  — sesi lain tetap hidup di background, tab bar muncul di halaman
  manapun yang lagi aktif.
- Parsing VTE/xterm **asli** lewat [`alacritty_terminal`](https://github.com/alacritty/alacritty)
  (bukan parser buatan sendiri) — 16 warna ANSI standar, color cube
  256, grayscale ramp, dan true color (`ESC[38;2;r;g;bm`).
  Pengaturan **font & color theme** bisa diganti langsung dari UI.
- Tidak ada local echo — persis terminal sungguhan, karakter yang
  tampil datang balik dari server lewat PTY.
- Host key server diverifikasi **trust-on-first-use** dan ditolak
  keras kalau berubah di koneksi berikutnya (proteksi MITM dasar).
- Auto-close tab kalau sesi diakhiri dari sisi remote (mis. ketik
  `exit`), tanpa perlu klik "×" manual.

### 📁 SFTP — Dual-Pane File Browser

- Setelah connect, tampil dua panel bersebelahan: **Local** (kiri) dan
  **Remote** (kanan), keduanya bisa navigasi folder & path bar yang
  bisa diketik langsung ke path manapun.
- **Drag & drop native** antar panel (Slint 1.17) untuk upload/
  download file — tidak perlu tombol panah terpisah.
- Menu klik-kanan / tombol "⋮" di tiap panel dan tiap baris
  file: **Rename**, **Delete**, **Refresh**, **New Folder**, **Show
  Hidden Files**, **Select All**.
- Multi-select file: klik biasa (toggle satu-satu), **Shift+klik** /
  **Shift+Panah** untuk seleksi rentang, **Ctrl+A** untuk pilih semua,
  klik area kosong untuk membatalkan seleksi.
- Shortcut **Ctrl+R** untuk refresh kedua panel.

### 🔌 Console — Koneksi Serial Lintas Platform

- Koneksi serial ke perangkat lewat kabel USB-to-serial,
  **cross-platform sejak awal** — bukan cuma `/dev/ttyUSB0`: otomatis
  mendeteksi nama port yang benar di Windows (`COM3`, dst), Linux
  (`/dev/ttyUSB0`, `/dev/ttyACM0`), maupun macOS
  (`/dev/cu.usbserial-*`).
- Dipakai untuk akses kabel console fisik (RS-232/USB-to-serial) ke
  perangkat jaringan (switch/router/access point apa pun yang punya
  port console serial) — terpisah total dari sesi SSH (beda transport,
  beda halaman).
- Pilih port yang terdeteksi + baud rate (default 9600, format 8-N-1 —
  setelan standar kabel console RS-232 kebanyakan perangkat jaringan),
  klik Connect — grid terminal ANSI-nya reuse rendering yang sama
  dengan tab SSH.

## Tumpukan Teknologi

| Lapisan | Pustaka | Kegunaan |
| --- | --- | --- |
| GUI | [Slint](https://slint.dev) 1.17 | UI declarative, native (bukan web view) |
| Async runtime | [Tokio](https://tokio.rs) | Semua I/O jaringan/serial/vault jalan di sini |
| SSH | [`russh`](https://github.com/Eugeny/russh) | Klien SSH murni Rust |
| SFTP | [`russh-sftp`](https://docs.rs/russh-sftp) | Subsystem SFTP di atas koneksi `russh` |
| Serial | [`tokio-serial`](https://docs.rs/tokio-serial) | Wrapper async lintas-platform di atas `serialport` |
| Emulasi terminal | [`alacritty_terminal`](https://github.com/alacritty/alacritty) | Parser VTE/xterm asli |
| Penyimpanan | [`rusqlite`](https://docs.rs/rusqlite) (bundled) | SQLite lokal, tanpa dependency sistem tambahan |
| Kripto | [`argon2`](https://docs.rs/argon2), [`chacha20poly1305`](https://docs.rs/chacha20poly1305) | Derive key master password + enkripsi AEAD |
| Import config | [`roxmltree`](https://docs.rs/roxmltree) | Parsing `config.xml` SecureCRT |

## Struktur Proyek

```text
terminus/
├── Cargo.toml                # workspace root — semua versi dependency dipusatkan di sini
├── crates/
│   ├── core/                  # domain model murni (HostProfile, HostGroup, dst) — tanpa I/O
│   ├── vault/                 # penyimpanan terenkripsi (Argon2id + ChaCha20-Poly1305)
│   ├── ssh-engine/            # transport SSH, wrapper russh
│   ├── sftp-engine/           # transfer file SFTP, wrapper russh-sftp (reuse koneksi ssh-engine)
│   ├── serial-engine/         # transport serial/console lintas-platform, wrapper tokio-serial
│   ├── term-emulator/         # parsing VTE (escape sequence) via alacritty_terminal
│   └── app/                   # binary utama: UI Slint + wiring semua crate di atas
├── ui/                        # file .slint (UI declarative, dicompile lewat build.rs di crates/app)
│   ├── tokens.slint            # design tokens: warna/tipografi/spacing/radius (1 sumber kebenaran)
│   ├── components/             # NavRail, TopBar, HostCard, FileRow, TerminalView, ConsoleView, dst
│   └── pages/                  # PageHosts, PageSftp, PageConsole, PageTerminal
└── docs/                       # tempat dokumentasi tambahan (screenshot, arsitektur) — kosong untuk saat ini
```

Alur dependency antar crate **searah** — `core` tidak pernah depend ke
crate lain, semua crate lain depend ke `core` (lihat
[Prinsip Desain](#prinsip-desain)).

### Prinsip Desain

- **Tiap crate = satu tanggung jawab.** `core` murni domain model tanpa
  I/O — ini menjaganya tetap ringan dan gampang ditest.
- **Tidak ada plaintext secret di luar `vault`.** `HostProfile` hanya
  menyimpan `credential_id` (UUID); nilai rahasianya hidup di
  `terminus-vault` dan hanya didecrypt saat runtime, tepat sebelum
  connect.
- **UI thread vs async runtime terpisah.** Slint jalan di main thread
  (event loop native OS); semua kerja jaringan/vault jalan di runtime
  Tokio terpisah, dikomunikasikan lewat `slint::invoke_from_event_loop`
  (lihat `crates/app/src/state.rs`).
- **UI = satu sumber kebenaran design token.** Semua warna/spacing/
  radius di `ui/**/*.slint` diambil dari `global Tokens` di
  `ui/tokens.slint` — tidak ada hex/px literal tersebar di komponen.

## Prasyarat & Instalasi

### 1. Rust Toolchain

Butuh Rust edisi 2021 terbaru (stable, minimal 1.85 — beberapa
dependency transitif mensyaratkan versi itu).

```bash
# Opsi A — rustup (disarankan, portable di Linux/macOS/Windows)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Opsi B — paket distro (contoh Fedora)
sudo dnf install rust cargo
```

Verifikasi: `rustc --version`.

### 2. Library Native untuk Slint (Rendering GUI)

Slint butuh toolchain C (linking) + library windowing/font sistem —
beda-beda per OS.

#### Linux

```bash
# Fedora
sudo dnf install gcc gcc-c++ pkgconf-pkg-config \
    fontconfig-devel libxkbcommon-devel wayland-devel libX11-devel

# Debian/Ubuntu
sudo apt install build-essential pkg-config \
    libfontconfig1-dev libxkbcommon-dev libwayland-dev libx11-dev

# Arch
sudo pacman -S base-devel pkgconf fontconfig libxkbcommon wayland libx11
```

Slint otomatis pilih backend Wayland kalau `$WAYLAND_DISPLAY` ada,
fallback ke X11 kalau tidak — kedua library di atas aman dipasang
dua-duanya, tidak perlu tahu compositor yang dipakai duluan.

#### macOS

Cukup **Xcode Command Line Tools**:

```bash
xcode-select --install
```

Slint pakai backend native macOS (AppKit/Metal lewat `winit`), tidak
butuh paket tambahan lain. Berjalan native di Apple Silicon (M1/M2/M3)
maupun Intel — Cargo otomatis compile sesuai arsitektur mesin yang
dipakai, tidak perlu flag khusus.

#### Windows

1. Pasang **Visual Studio Build Tools** (bukan Visual Studio penuh,
   cukup Build Tools-nya) dari
   [visualstudio.microsoft.com/downloads](https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022) —
   waktu instalasi, **wajib centang workload "Desktop development with
   C++"** (menyertakan MSVC linker `link.exe` yang dibutuhkan Rust
   target `x86_64-pc-windows-msvc`).
2. Pasang Rust lewat [rustup-init.exe](https://rustup.rs) (opsi A di
   atas) — installer-nya otomatis mendeteksi & memakai toolchain MSVC
   yang baru dipasang.

```powershell
# Verifikasi keduanya sudah kepasang & saling terhubung
rustc --version
cargo --version
```

Slint pakai backend native Win32/Direct3D (lewat `winit` +
`i-slint-renderer-femtovg`), **tidak butuh WebView2** atau runtime GUI
tambahan apa pun — beda dari framework berbasis web view (Tauri/
Electron).

> `rusqlite` dipakai dengan feature `bundled` — SQLite di-compile dari
> source lewat `cc`, jadi **tidak perlu** install `sqlite-devel`/
> `libsqlite3-dev` terpisah di platform manapun (termasuk Windows —
> `cc` otomatis pakai toolchain MSVC yang sudah dipasang di atas).

### 3. Font (Opsional, Kosmetik)

Desain (`ui/tokens.slint`) merujuk **Hanken Grotesk** (headline),
**Inter** (body), dan **JetBrains Mono** (data teknis & terminal).
Kalau tidak terinstall, Slint otomatis fallback ke font default sistem
tanpa error — tampilan tetap rapi.

```bash
# Fedora — Inter & JetBrains Mono tersedia di repo resmi
sudo dnf install rsms-inter-fonts jetbrains-mono-fonts-all

# Hanken Grotesk tidak dipaketkan distro manapun — download manual
# dari https://fonts.google.com/specimen/Hanken+Grotesk kalau mau
# match persis; headline tetap terbaca jelas tanpa itu.
```

## Menjalankan Aplikasi

```bash
# Build seluruh workspace (build pertama lebih lama, ~2-5 menit,
# menarik & compile ratusan crate — termasuk Slint dan russh)
cargo build --workspace

# Jalankan binary utama
cargo run -p terminus-app
```

Saat pertama kali dijalankan, aplikasi akan meminta membuat **master
password** (dipakai buat derive key enkripsi vault) — password ini
**tidak tersimpan di mana pun** dan **tidak bisa dipulihkan** kalau
lupa (by design, sama seperti password manager pada umumnya).

### Iterasi Cepat Khusus Desain UI (Tanpa Rebuild Rust)

Untuk mengubah file `.slint` saja (warna, layout, komponen baru) tanpa
menunggu seluruh workspace Rust ikut terbuild ulang, pakai
`slint-viewer` — tool preview standalone dari tim Slint:

```bash
# Install sekali (samakan versi dengan `slint.workspace` di Cargo.toml)
cargo install slint-viewer --version 1.17.1 --locked

# Buka window live-preview, auto-reload tiap file .slint disimpan
slint-viewer --auto-reload ui/app-window.slint

# Compile-check tanpa buka window (cocok untuk CI/pre-commit)
slint-viewer --check ui/app-window.slint

# Render satu halaman ke PNG tanpa GUI
slint-viewer --screenshot out.png ui/app-window.slint
```

## Menjalankan Test

```bash
# Semua unit test lintas-crate (cepat, tidak butuh jaringan/hardware)
cargo test --workspace
```

Dua test integrasi CONNECT SUNGGUHAN (bukan mock) ke server SSH lokal
dikecualikan secara default (`#[ignore]`) karena butuh Docker:

```bash
docker run -d --name terminus-test-sshd -p 2222:22 alpine:3.20 sh -c "
  apk add --no-cache openssh >/dev/null 2>&1
  echo 'root:testpass123' | chpasswd
  sed -i 's/#PermitRootLogin.*/PermitRootLogin yes/' /etc/ssh/sshd_config
  sed -i 's/#PasswordAuthentication.*/PasswordAuthentication yes/' /etc/ssh/sshd_config
  ssh-keygen -A >/dev/null 2>&1
  /usr/sbin/sshd -D
"
cargo test -p terminus-ssh-engine --test end_to_end -- --ignored
cargo test -p terminus-app --test ssh_terminal_e2e -- --ignored

# beres testing:
docker rm -f terminus-test-sshd
```

## Packaging & Rilis

Build binary rilis jadi file executable siap-distribusi, per OS —
script-nya ada di [`packaging/`](packaging/):

```bash
# Linux — .deb + .rpm + AppImage sekaligus, hasil masuk build/
./packaging/linux/build-all.sh

# macOS — .app + .dmg (WAJIB dijalankan di Mac asli, tidak bisa
# cross-compile dari Linux/Windows, lihat komentar di script-nya)
./packaging/macos/build.sh
```

```powershell
# Windows — .exe + .zip (WAJIB dijalankan di Windows asli, PowerShell)
.\packaging\windows\build.ps1
```

Semua hasil masuk ke folder `build/` di root proyek. Detail lengkap
(dependency yang dibutuhkan tiap script, catatan code-signing/
notarization yang belum dikerjakan) ada di
[`packaging/README.md`](packaging/README.md).

## Isu Dependency yang Sudah Diperbaiki

`terminus-ssh-engine` depend ke `russh 0.58`, yang secara transitif
menarik `rsa 0.10.0-rc.12` — sebuah **release-candidate** yang
mensyaratkan versi pre-release spesifik tiga crate RustCrypto lain
(`pkcs8 0.11.0-rc.8`, `spki 0.8.0-rc.4`, `der 0.8.0-rc.9`). Kalau
resolver Cargo dibiarkan pilih bebas (`cargo update` tanpa `-p`, atau
`Cargo.lock` dihapus), dia akan menarik versi **final** yang tidak
kompatibel dan build akan gagal.

**Fix-nya sudah dipin di `Cargo.lock`** (jangan dihapus/di-gitignore).
Kalau suatu saat kejadian lagi (habis `cargo update` penuh, atau
upgrade `russh`):

```bash
cargo update -p pkcs8 --precise 0.11.0-rc.8
cargo update -p spki --precise 0.8.0-rc.4
cargo update -p der --precise 0.8.0-rc.9
cargo build --workspace   # pastikan hijau lagi
```

(Urutan penting: `spki` harus dipin dulu sebelum `der`.)

## Status & Roadmap

**Sudah selesai & berfungsi penuh:**

- ✅ Vault terenkripsi (Argon2id + ChaCha20-Poly1305)
- ✅ Manajemen Host & Grup (CRUD, panel detail, duplicate, search, import SecureCRT)
- ✅ Terminal SSH multi-tab (parsing VTE asli, warna penuh, host-key TOFU, tema per-host)
- ✅ SFTP dual-pane (drag & drop, context menu, multi-select, path bar editable)
- ✅ Console serial lintas platform (Windows/Linux/macOS), disconnect yang benar-benar memutus
- ✅ Tema per-host persist lintas restart app (kolom `terminal_theme` di skema `HostProfile`,
  migrasi vault otomatis buat instalasi lama)
- ✅ Export host ke `config.xml` format SecureCRT (kebalikan dari Import — folder grup ikut
  ter-export bertingkat, password sengaja TIDAK ikut, sama seperti Import)
- ✅ Export backup JSON TERMASUK password, dienkripsi Argon2id+ChaCha20-Poly1305 pakai
  passphrase backup terpisah (bukan master password vault)
- ✅ Identity tersimpan (pasangan username+password terpisah dari host) — dikelola lewat
  dialog "Identities", dipilih dari panel "New Host" buat isi otomatis (deep copy, bukan
  referensi hidup)

**Batasan saat ini:**

- ⏳ Ukuran PTY terminal masih tetap (100×32), belum reflow mengikuti
  ukuran jendela; belum ada scrollback (hanya viewport aktif).
- ⏳ Console (serial) belum punya konsep "saved device profile" — port
  dipilih ulang tiap sesi (disengaja, lihat komentar di
  `ui/pages/page-console.slint`).

**Rencana selanjutnya:**

1. **API backend + aplikasi Android** yang sinkron dengan vault desktop.

## Berkontribusi

Kontribusi dalam bentuk apa pun — laporan bug, ide fitur, maupun pull
request — dipersilakan.

> **Aturan wajib:** **dilarang push langsung ke `main`** — SEMUA
> perubahan (termasuk dari maintainer) harus lewat **Pull Request**
> dan direview dulu sebelum di-merge. Ini berlaku tanpa terkecuali.

1. **Fork** repo ini, buat branch baru dari `main` (jangan commit
   langsung ke `main`).
2. Jalankan `cargo build --workspace` dan `cargo test --workspace`
   sebelum membuka PR — pastikan keduanya hijau.
3. Kalau mengubah file `.slint`, jalankan `slint-viewer --check
   ui/app-window.slint` juga.
4. Ikuti konvensi yang sudah ada di kode:
   - Komentar ditulis dalam **Bahasa Indonesia**, fokus menjelaskan
     **kenapa** suatu keputusan diambil (bukan cuma mengulang apa yang
     kodenya sudah jelas lakukan).
   - Jangan tulis warna/spacing/radius literal di `.slint` — selalu
     lewat `global Tokens` (`ui/tokens.slint`).
   - Jaga arah dependency antar crate tetap searah (lihat
     [Prinsip Desain](#prinsip-desain)) — jangan sampai `core` depend
     balik ke crate lain.
   - Kredensial/secret tidak boleh mendarat di `core` atau logging
     mana pun — hanya lewat `terminus-vault`.
5. Buka **Pull Request** ke branch `main`, deskripsikan dengan jelas:
   masalah apa yang diselesaikan, dan bagaimana cara mengetesnya secara
   manual. Tunggu review — PR baru di-merge setelah disetujui.

Detail lengkap ada di [`CONTRIBUTING.md`](CONTRIBUTING.md).

Dengan membuka pull request ke repo ini, kamu setuju kontribusimu
dilisensikan di bawah lisensi yang sama dengan proyek ini (lihat di
bawah).

## Lisensi

Proyek ini dilisensikan di bawah **GNU General Public License v3.0 or
later (GPL-3.0-or-later)** — lihat deklarasi lisensi di
`Cargo.toml` (`workspace.package.license`). Ringkasnya: bebas
digunakan, dipelajari, dimodifikasi, dan didistribusikan ulang, selama
turunannya tetap open source di bawah lisensi yang sama.
