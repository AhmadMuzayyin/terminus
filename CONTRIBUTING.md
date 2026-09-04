# Berkontribusi ke Terminus

Terima kasih sudah tertarik berkontribusi! Kontribusi dalam bentuk
apa pun — laporan bug, ide fitur, perbaikan dokumentasi, maupun pull
request kode — dipersilakan.

## Aturan Wajib: Pull Request, Tanpa Terkecuali

**Dilarang push langsung ke branch `main`.** Semua perubahan —
termasuk dari maintainer proyek ini sendiri — **wajib** lewat Pull
Request dan direview dulu sebelum di-merge. Tidak ada pengecualian,
sekecil apa pun perubahannya (termasuk typo fix).

Alasannya sederhana: `main` adalah sumber kebenaran yang orang lain
`fork`/`pull`/pakai buat build rilis — riwayat commit yang bersih &
terreview jauh lebih penting daripada kecepatan sesaat.

## Alur Kontribusi

1. **Fork** repository ini.
2. Buat branch baru dari `main` (nama bebas, tapi deskriptif — mis.
   `fix/sftp-drag-drop` atau `feat/private-key-auth`).
3. Kerjakan perubahanmu di branch itu.
4. Sebelum membuka PR, pastikan:

   ```bash
   cargo build --workspace   # harus hijau
   cargo test --workspace    # harus hijau
   ```

5. Kalau perubahanmu menyentuh file `.slint`, jalankan juga:

   ```bash
   slint-viewer --check ui/app-window.slint
   ```

6. Push branch-mu, buka **Pull Request** ke `main` di repo asli
   (bukan fork-mu). Deskripsikan dengan jelas:
   - Masalah apa yang diselesaikan / fitur apa yang ditambahkan.
   - Bagaimana cara mengetesnya secara manual (langkah reproduksi).
   - Screenshot/GIF kalau perubahannya menyangkut UI.
7. Tunggu review. PR baru di-merge setelah disetujui — mungkin ada
   permintaan perubahan, itu bagian normal dari proses.

## Konvensi Kode

- Komentar ditulis dalam **Bahasa Indonesia**, fokus menjelaskan
  **kenapa** suatu keputusan diambil — bukan cuma mengulang apa yang
  kodenya sudah jelas lakukan.
- Jangan tulis warna/spacing/radius literal di file `.slint` — selalu
  lewat `global Tokens` (`ui/tokens.slint`). Kalau mau ganti tema,
  cukup ubah satu file itu.
- Jaga arah dependency antar crate tetap **searah**: `crates/core`
  tidak boleh depend ke crate lain mana pun (lihat
  [docs/architecture](docs/architecture/README.md)).
- Kredensial/secret (password, private key, dst) tidak boleh mendarat
  di `terminus-core` atau kena `log`/`println!` di mana pun — cuma
  boleh lewat `terminus-vault` (terenkripsi at-rest).

## Melaporkan Bug / Mengusulkan Fitur

Buka [issue baru](https://github.com/AhmadMuzayyin/terminus/issues).
Untuk bug, sertakan:

- Langkah reproduksi.
- OS/distro + versi Rust (`rustc --version`).
- Output error lengkap (jangan dipotong).

## Lisensi

Proyek ini dilisensikan di bawah **GPL-3.0-or-later**. Dengan membuka
pull request ke repo ini, kamu setuju kontribusimu dilisensikan di
bawah lisensi yang sama.
