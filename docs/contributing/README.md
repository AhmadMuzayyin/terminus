# Berkontribusi

Kontribusi dalam bentuk apa pun — laporan bug, ide fitur, maupun pull
request — dipersilakan. Detail lengkap ada di
[`CONTRIBUTING.md`](https://github.com/AhmadMuzayyin/terminus/blob/main/CONTRIBUTING.md)
di root repo; ringkasannya di bawah.

> [!IMPORTANT]
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

## Menulis Dokumentasi

Tiap halaman di `docs/` = satu folder + `README.md`, didaftarkan di
array `NAV` pada `docs/index.html`. Markdown biasa (GFM) berlaku, plus
beberapa konvensi yang dirender khusus oleh situs dokumentasi (dan
tetap terbaca wajar di GitHub).

**Callout** — sintaks alert GitHub: `NOTE`, `TIP`, `IMPORTANT`,
`WARNING`, `CAUTION`.

```markdown
> [!WARNING]
> Mengganti host key lama tanpa memeriksa sidik jari baru membuka celah
> serangan MITM.
```

**Perintah shell** — pakai `bash` (atau `powershell`). Tulis perintahnya
saja tanpa `$`: prompt digambar otomatis dan tidak ikut tersalin.

**Sesi terminal (perintah + output)** — pakai `console`, tulis prompt
apa adanya. Baris berprompt = perintah, sisanya ditampilkan sebagai
output; tombol Salin hanya menyalin perintahnya.

```console
$ ssh -p 2222 noc-admin@192.0.2.10
noc-admin@192.0.2.10's password:
Last login: Tue Sep 30 09:12:44 2026 from 203.0.113.5
```

**Perangkat jaringan** — `cisco`, `huawei`, atau `routeros`
(MikroTik). Dengan prompt = sesi; tanpa prompt = potongan konfigurasi.

```cisco
Router# configure terminal
Router(config)# ip ssh version 2
Router(config)# line vty 0 4
Router(config-line)# transport input ssh
```

```huawei
[Huawei] stelnet server enable
[Huawei] user-interface vty 0 4
[Huawei-ui-vty0-4] authentication-mode aaa
[Huawei-ui-vty0-4] protocol inbound ssh
```

```routeros
[admin@MikroTik] > /ip service set ssh port=2222
[admin@MikroTik] > /ip service print where name=ssh
```

**Tab platform/perangkat** — blok kode berurutan dengan `tab="Nama"`
digabung jadi satu tab. Pilihan pembaca tersinkron ke semua tab
berlabel sama.

````markdown
```bash tab="Fedora"
sudo dnf install openssh-clients
```

```bash tab="Debian/Ubuntu"
sudo apt install openssh-client
```
````

Pakai alamat contoh dari rentang dokumentasi resmi (`192.0.2.0/24`,
`198.51.100.0/24`, `203.0.113.0/24`) — **jangan** tempel IP, hostname,
atau username infrastruktur sungguhan.

## Lisensi

Proyek ini dilisensikan di bawah **GNU General Public License v3.0 or
later (GPL-3.0-or-later)**. Ringkasnya: bebas digunakan, dipelajari,
dimodifikasi, dan didistribusikan ulang, selama turunannya tetap open
source di bawah lisensi yang sama.

## Butuh Bantuan?

Buka [issue baru](https://github.com/AhmadMuzayyin/terminus/issues) di
GitHub — sertakan langkah reproduksi, OS/distro, dan output error kalau
ada.
