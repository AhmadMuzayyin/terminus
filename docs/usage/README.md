# Menjalankan Aplikasi

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

## Iterasi Cepat Khusus Desain UI (Tanpa Rebuild Rust)

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

## Alur Pemakaian Singkat

1. Buka app → buat/unlock master password.
2. Halaman **Hosts** → `+ New host` untuk tambah koneksi SSH, atau
   `Import` untuk migrasi dari config XML VanDyke SecureCRT.
3. Klik satu kartu host → panel **Host Details** muncul di kanan →
   tombol **Connect** → tab terminal baru terbuka.
4. Halaman **SFTP** → pilih host tersimpan (atau `+ Manual`) → dua
   panel Local/Remote, drag & drop file antar panel.
5. Halaman **Console** → pilih port serial USB-to-serial yang
   terdeteksi + baud rate → `Connect` — untuk akses kabel console fisik
   ke perangkat jaringan (switch/router/access point).

Detail tiap fitur ada di halaman **[Fitur](#features)**.
