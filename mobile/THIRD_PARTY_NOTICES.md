# Pihak Ketiga di Terminus Mobile

Aplikasi mobile ini berlisensi GPL-3.0-or-later (sama dengan repo). Kode
pihak ketiga yang **disalin ke dalam repo** (bukan sekadar dependency
Gradle) dicatat di sini beserta perubahannya.

## terminal-view (Termux)

- Sumber: https://github.com/termux/termux-app, tag `v0.118.3`, folder
  `terminal-view/`.
- Lisensi: Apache License 2.0 (pengecualian dari lisensi GPLv3 repo
  termux-app — lihat `LICENSE.md` di repo tersebut; kode berasal dari
  "Terminal Emulator for Android" oleh Jack Palevich).
- Lokasi di sini: `app/src/main/java/com/termux/view/` dan
  `app/src/main/res/drawable/text_select_handle_*_material.xml`.
- Perubahan (juga ditulis di kepala tiap file):
  - Tipe `com.termux.terminal.TerminalSession` diganti
    `com.termux.view.TerminalViewSession` (kelas baru, BUKAN dari Termux)
    supaya view bisa menampilkan sesi SSH, bukan cuma proses lokal.
  - Import `com.termux.view.R` diganti `org.terminus.mobile.R`; teks menu
    seleksi diterjemahkan ke Bahasa Indonesia (`res/values/strings.xml`).

Dependency Gradle `com.github.termux.termux-app:terminal-emulator:v0.118.3`
(Apache License 2.0) dipakai apa adanya lewat JitPack.
