# Status & Roadmap

## Sudah selesai & berfungsi penuh

- ✅ Vault terenkripsi (Argon2id + ChaCha20-Poly1305)
- ✅ Manajemen Host & Grup (CRUD, panel detail, duplicate, search, import SecureCRT)
- ✅ Terminal SSH multi-tab (parsing VTE asli, warna penuh, host-key TOFU, tema per-host)
- ✅ SFTP dual-pane (drag & drop, context menu, multi-select, path bar editable)
- ✅ Console serial lintas platform (Windows/Linux/macOS), disconnect yang benar-benar memutus
- ✅ Layar "Connecting" terpadu untuk semua alur (SSH/SFTP/Console), termasuk prompt password
  otomatis kalau kredensial belum diisi
- ✅ Packaging tiap OS: `.deb`/`.rpm`/AppImage (Linux), `.app`/`.dmg` (macOS), `.exe`/`.zip` (Windows)
- ✅ Tema per-host persist lintas restart app (kolom `terminal_theme` di skema `HostProfile`,
  migrasi vault otomatis buat instalasi lama)

## Batasan saat ini

- ⏳ Ukuran PTY terminal masih tetap (100×32), belum reflow mengikuti
  ukuran jendela; belum ada scrollback (hanya viewport aktif).
- ⏳ Console (serial) belum punya konsep "saved device profile" — port
  dipilih ulang tiap sesi (disengaja — path port sering berubah antar
  colok-cabut/reboot).
- ⏳ Paket `.deb`/`.rpm`/`.exe` belum diverifikasi instal di
  Debian/Ubuntu/Fedora/Windows asli; `.app`/`.dmg`/`.exe` belum
  di-codesign/notarize.

## Rencana Selanjutnya

1. **Export host ke XML** (format serupa `config.xml` SecureCRT) —
   kebalikan dari fitur Import yang sudah ada.
2. **Export JSON dengan password terenkripsi** — format backup/
   pindahan portable, terpisah dari file vault SQLite asli.
3. **Identity tersimpan** (pasangan username+password terpisah dari
   host) — bisa dipakai ulang sebagai kredensial waktu menambah host
   baru, tanpa isi ulang dari nol tiap kali.
4. **API backend + aplikasi Android** yang sinkron dengan vault desktop.

Lihat **[Berkontribusi](#contributing)** untuk cara mulai bantu
kerjakan salah satunya.
