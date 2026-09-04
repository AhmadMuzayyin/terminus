# Berkontribusi

Kontribusi dalam bentuk apa pun — laporan bug, ide fitur, maupun pull
request — dipersilakan. Detail lengkap ada di
[`CONTRIBUTING.md`](https://github.com/AhmadMuzayyin/terminus/blob/main/CONTRIBUTING.md)
di root repo; ringkasannya di bawah.

> **Aturan wajib:** **dilarang push langsung ke `main`** — SEMUA
> perubahan (termasuk dari maintainer) harus lewat **Pull Request**
> dan direview dulu sebelum di-merge, tanpa terkecuali.

1. **Fork** repo ini, buat branch baru dari `main` (jangan commit
   langsung ke `main`).
2. Jalankan `cargo build --workspace` dan `cargo test --workspace`
   sebelum membuka PR — pastikan keduanya hijau (lihat
   **[Testing](#testing)**).
3. Kalau mengubah file `.slint`, jalankan `slint-viewer --check
   ui/app-window.slint` juga (lihat **[Menjalankan Aplikasi](#usage)**).
4. Ikuti konvensi yang sudah ada di kode:
   - Komentar ditulis dalam **Bahasa Indonesia**, fokus menjelaskan
     **kenapa** suatu keputusan diambil (bukan cuma mengulang apa yang
     kodenya sudah jelas lakukan).
   - Jangan tulis warna/spacing/radius literal di `.slint` — selalu
     lewat `global Tokens` (`ui/tokens.slint`).
   - Jaga arah dependency antar crate tetap searah (lihat
     **[Arsitektur](#architecture)**) — jangan sampai `core` depend
     balik ke crate lain.
   - Kredensial/secret tidak boleh mendarat di `core` atau logging
     mana pun — hanya lewat `terminus-vault`.
5. Buka **Pull Request** ke `main`, deskripsikan dengan jelas: masalah
   apa yang diselesaikan, dan bagaimana cara mengetesnya secara
   manual. Tunggu review — PR baru di-merge setelah disetujui.

Dengan membuka pull request ke repo ini, kamu setuju kontribusimu
dilisensikan di bawah lisensi yang sama dengan proyek ini.

## Lisensi

Proyek ini dilisensikan di bawah **GNU General Public License v3.0 or
later (GPL-3.0-or-later)**. Ringkasnya: bebas digunakan, dipelajari,
dimodifikasi, dan didistribusikan ulang, selama turunannya tetap open
source di bawah lisensi yang sama.

## Butuh Bantuan?

Buka [issue baru](https://github.com/AhmadMuzayyin/terminus/issues) di
GitHub — sertakan langkah reproduksi, OS/distro, dan output error kalau
ada.
