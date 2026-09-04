# Fitur

## 🔐 Manajemen Host & Vault Terenkripsi

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
  tags/grup langsung di situ, tombol **Duplicate**, reveal/hide
  password (👁), dan konfirmasi eksplisit sebelum menghapus.
- Kalau password host belum diisi (mis. hasil import), begitu klik
  **Connect** aplikasi otomatis minta password di layar loading —
  submit langsung tersimpan ke vault **dan** langsung login, tidak
  perlu bolak-balik ke panel Host Details dulu.
- Pencarian global (kotak di TopBar) menembus semua halaman & semua
  grup — cocok di label, host/IP, username, `user@host`, atau tags.
- **Import dari [VanDyke SecureCRT](https://www.vandyke.com/products/securecrt/)**
  (`config.xml`) — folder bertingkat di-flatten jadi nama grup gabungan,
  dedup otomatis kalau diimpor ulang. Password **tidak pernah** ikut
  diimpor (terenkripsi proprietary VanDyke, secara sengaja tidak
  dibongkar).

## 💻 Terminal SSH Interaktif

- Connect ke sebuah host membuka **tab terminal baru** (mirip browser)
  — sesi lain tetap hidup di background, tab bar muncul di halaman
  manapun yang lagi aktif.
- Parsing VTE/xterm **asli** lewat
  [`alacritty_terminal`](https://github.com/alacritty/alacritty) —
  16 warna ANSI standar, color cube 256, grayscale ramp, dan true
  color. **Font & color theme** bisa diganti langsung dari UI, dan
  **tersimpan per host** — tiap host bisa punya tema warna sendiri,
  otomatis dipakai lagi tiap connect ulang ke host itu.
- Tidak ada local echo — persis terminal sungguhan, karakter yang
  tampil datang balik dari server lewat PTY.
- Host key server diverifikasi **trust-on-first-use** dan ditolak
  keras kalau berubah di koneksi berikutnya (proteksi MITM dasar).
- Auto-close tab kalau sesi diakhiri dari sisi remote (mis. ketik
  `exit`), tanpa perlu klik "×" manual.

## 📁 SFTP — Dual-Pane File Browser

- Setelah connect, tampil dua panel bersebelahan: **Local** (kiri) dan
  **Remote** (kanan), keduanya bisa navigasi folder & path bar yang
  bisa diketik langsung ke path manapun.
- **Drag & drop native** antar panel (Slint 1.17) untuk upload/
  download file.
- Menu klik-kanan / tombol "⋮" di tiap panel dan tiap baris
  file: **Rename**, **Delete**, **Refresh**, **New Folder**, **Show
  Hidden Files**, **Select All**.
- Multi-select file: klik biasa (toggle satu-satu), **Shift+klik** /
  **Shift+Panah** untuk seleksi rentang, **Ctrl+A** untuk pilih semua,
  klik area kosong untuk membatalkan seleksi.
- Shortcut **Ctrl+R** untuk refresh kedua panel.
- Kalau belum ada host tersimpan sama sekali, halaman menampilkan
  pesan yang jelas (bukan area kosong tanpa penjelasan).

## 🔌 Console — Koneksi Serial Lintas Platform

- Koneksi serial ke perangkat lewat kabel USB-to-serial,
  **cross-platform sejak awal** — otomatis mendeteksi nama port yang
  benar di Windows (`COM3`, dst), Linux (`/dev/ttyUSB0`), maupun macOS
  (`/dev/cu.usbserial-*`).
- Daftar port **disaring ke adaptor USB-to-serial beneran saja** (VID/
  PID adaptor fisik) — port bawaan sistem yang tidak relevan (mis.
  `/dev/ttyS0`..`/dev/ttyS31` di Linux) otomatis disembunyikan.
- Pilih port + baud rate (default 9600, format 8-N-1 — setelan standar
  kabel console RS-232 kebanyakan perangkat jaringan), klik Connect —
  grid terminal ANSI-nya independen dari tema tab SSH manapun (tidak
  pernah "ketuker" warnanya).
- **Disconnect benar-benar memutus koneksi** di level OS (bukan cuma
  kosmetik) — connect ulang selalu berarti login dari awal lagi, sama
  seperti mencabut-colok kabel fisik.
