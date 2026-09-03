# Packaging

Script buat bungkus binary Terminus jadi paket distributable. Semua
hasil masuk ke `build/` di root proyek (tidak ke-commit, lihat
`.gitignore`).

## Linux (bisa dijalankan di mesin manapun, termasuk sandbox ini)

```bash
# Semua sekaligus (.deb + .rpm + AppImage):
./packaging/linux/build-all.sh

# Atau satu-satu (asumsikan `cargo build --release -p terminus-app`
# sudah dijalankan duluan):
./packaging/linux/build-deb.sh
./packaging/linux/build-rpm.sh
./packaging/linux/build-appimage.sh
```

Hasil: `build/terminus_0.1.0_amd64.deb`, `build/terminus-0.1.0-1.*.rpm`,
`build/Terminus-x86_64.AppImage`.

Catatan:
- `.deb`/`.rpm` **belum diverifikasi instal** di Debian/Ubuntu/Fedora
  beneran (list dependency-nya "best effort" dari `ldd`, lihat komentar
  di `build-deb.sh`) — coba instal & jalankan sebelum didistribusikan
  luas.
- `build-appimage.sh` mengunduh `linuxdeploy`/`appimagetool` (sekali,
  di-cache ke `packaging/linux/tools/`) — butuh akses internet.

## macOS (`.app` + `.dmg`) — WAJIB dijalankan di Mac asli

**Tidak bisa dibangun dari lingkungan ini (Linux)** — Slint/winit link
langsung ke framework Cocoa/AppKit macOS, yang cuma tersedia kalau
OS-nya beneran macOS (bukan sekadar target-triple Rust; Apple tidak
mendistribusikan SDK-nya buat Linux). Salin proyek ini ke Mac (atau
pakai runner CI `macos-latest`), lalu:

```bash
./packaging/macos/build.sh
```

Hasil: `build/Terminus.app`, `build/Terminus-0.1.0.dmg`. Binary belum
di-codesign/notarize (lihat catatan di ujung output script) — cukup
buat testing lokal, distribusi publik butuh Apple Developer ID.
