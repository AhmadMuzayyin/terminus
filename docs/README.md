# Terminus

> SSH · SFTP · Console (serial) manager desktop native, lintas platform,
> ditulis 100% [Rust](https://www.rust-lang.org/) dengan GUI
> [Slint](https://slint.dev) — open-source, satu binary, tanpa
> Electron, tanpa runtime tambahan.

Terminus adalah aplikasi manajemen koneksi remote (SSH, SFTP, dan
console serial) yang dibangun sebagai aplikasi desktop
native. Seluruh logic (kripto, transport SSH/SFTP/serial, parsing
terminal) ditulis di Rust dan dites langsung tanpa lapisan JavaScript,
sementara UI-nya dideklarasikan lewat [Slint](https://slint.dev) —
toolkit GUI native yang di-compile jadi kode Rust saat build.

Proyek ini open source di bawah lisensi **GPL-3.0-or-later** — silakan
di-*fork*, dipelajari, dan dikontribusikan.

## Mulai dari mana?

- **[Instalasi](#getting-started)** — pasang Rust toolchain + library
  native yang dibutuhkan Slint, per platform (Linux/macOS/Windows).
- **[Menjalankan Aplikasi](#usage)** — build & jalankan dari source,
  plus iterasi cepat khusus desain UI.
- **[Fitur](#features)** — ringkasan semua fitur yang sudah berfungsi
  penuh: vault terenkripsi, terminal SSH, SFTP, console serial.
- **[Arsitektur](#architecture)** — struktur workspace crate, prinsip
  desain, tumpukan teknologi.
- **[Packaging & Rilis](#packaging)** — build executable siap-distribusi
  di tiap OS: `.deb`/`.rpm`/AppImage (Linux), `.app`/`.dmg` (macOS),
  `.exe`/`.zip` (Windows).
- **[Berkontribusi](#contributing)** — cara kontribusi, konvensi kode,
  lisensi.

## Tautan cepat

- Kode sumber: [github.com/AhmadMuzayyin/terminus](https://github.com/AhmadMuzayyin/terminus)
- Laporkan bug/ide fitur: [GitHub Issues](https://github.com/AhmadMuzayyin/terminus/issues)
- README lengkap (versi satu-halaman): [README.md di repo](https://github.com/AhmadMuzayyin/terminus/blob/main/README.md)
