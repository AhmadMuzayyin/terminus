# Packaging & Rilis

Script buat bungkus binary Terminus jadi paket distributable ada di
[`packaging/`](https://github.com/AhmadMuzayyin/terminus/tree/main/packaging)
— hasilnya masuk ke folder `build/` di root proyek (tidak ikut
ke-commit).

## Linux (`.deb`, `.rpm`, AppImage)

Bisa dijalankan di mesin Linux manapun:

```bash
# Semua sekaligus:
./packaging/linux/build-all.sh

# Atau satu-satu (asumsikan `cargo build --release -p terminus-app`
# sudah dijalankan duluan):
./packaging/linux/build-deb.sh
./packaging/linux/build-rpm.sh
./packaging/linux/build-appimage.sh
```

Hasil: `build/terminus_0.1.0_amd64.deb`, `build/terminus-0.1.0-1.*.rpm`,
`build/Terminus-x86_64.AppImage`.

- `.deb`/`.rpm` disusun manual lewat `dpkg-deb`/`rpmbuild` (bukan
  `cargo-deb`/`cargo-generate-rpm`) — daftar dependency-nya "best
  effort" dari `ldd` binary rilisnya, sebaiknya dites instal dulu di
  Debian/Ubuntu/Fedora asli sebelum didistribusikan luas.
- AppImage disusun lewat `linuxdeploy` + `appimagetool` (didownload
  otomatis sekali, di-cache di `packaging/linux/tools/`, butuh akses
  internet) — otomatis bundling semua shared library dependency biar
  portable.

## macOS (`.app`, `.dmg`) — WAJIB dijalankan di Mac asli

**Tidak bisa dibangun dari Linux** — Slint/winit di macOS link
langsung ke framework Cocoa/AppKit, yang cuma tersedia kalau OS-nya
beneran macOS. Di Mac (atau runner CI `macos-latest`):

```bash
./packaging/macos/build.sh
```

Hasil: `build/Terminus.app`, `build/Terminus-0.1.0.dmg`. Binary belum
di-codesign/notarize — cukup buat testing lokal; distribusi publik
tanpa peringatan Gatekeeper butuh Apple Developer ID.

## Windows (`.exe`, `.zip`) — WAJIB dijalankan di Windows asli

**Tidak bisa dibangun dari Linux/macOS** — link ke toolchain MSVC yang
cuma ada di Windows (lihat [Instalasi](../getting-started/)). Di
Windows (PowerShell), atau runner CI `windows-latest`:

```powershell
.\packaging\windows\build.ps1
```

Hasil: `build\terminus.exe`, `build\terminus-0.1.0-windows-x86_64.zip`.
Windows tidak butuh format "paket" berlapis ala Linux — satu `.exe`
sudah langsung bisa dijalankan/dibagikan apa adanya. Binary belum
di-codesign — Windows SmartScreen mungkin menandainya "unrecognized
app" di mesin lain (tetap bisa dijalankan lewat "More info" → "Run
anyway"); installer MSI/NSIS belum dibuat.

## Ikon

Ikon aplikasi ("T" biru, konsisten dengan avatar di dalam UI) ada di
`packaging/linux/icons/` (PNG berbagai ukuran) — dipakai ulang oleh
ketiga platform (dikonversi ke `.icns` on-the-fly oleh
`packaging/macos/build.sh` di Mac).
