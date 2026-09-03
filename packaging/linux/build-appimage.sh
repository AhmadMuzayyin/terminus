#!/usr/bin/env bash
# Bungkus binary rilis jadi AppImage portable (x86_64) — pakai
# `linuxdeploy` (nge-bundle SEMUA shared library dependency binary-nya
# otomatis, lihat `ldd` di komentar `build-deb.sh` buat daftar yang
# ke-deteksi) + `appimagetool` (bungkus AppDir jadi satu file
# .AppImage). Dua tool itu didownload sekali ke `packaging/linux/tools/`
# (di-gitignore, bukan bagian source proyek) kalau belum ada.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BIN="$ROOT/target/release/terminus"
TOOLS="$ROOT/packaging/linux/tools"
APPDIR="$ROOT/build/AppDir"

if [ ! -f "$BIN" ]; then
    echo "Binary rilis belum ada — jalankan dulu: cargo build --release -p terminus-app" >&2
    exit 1
fi

mkdir -p "$TOOLS"
LINUXDEPLOY="$TOOLS/linuxdeploy-x86_64.AppImage"
APPIMAGETOOL="$TOOLS/appimagetool-x86_64.AppImage"

if [ ! -f "$LINUXDEPLOY" ]; then
    echo "Mengunduh linuxdeploy..."
    curl -fL -o "$LINUXDEPLOY" "https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage"
    chmod +x "$LINUXDEPLOY"
fi
if [ ! -f "$APPIMAGETOOL" ]; then
    echo "Mengunduh appimagetool..."
    curl -fL -o "$APPIMAGETOOL" "https://github.com/AppImage/AppImageKit/releases/download/continuous/appimagetool-x86_64.AppImage"
    chmod +x "$APPIMAGETOOL"
fi

rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin" "$APPDIR/usr/share/applications" "$APPDIR/usr/share/icons/hicolor/512x512/apps"
install -m 0755 "$BIN" "$APPDIR/usr/bin/terminus"
install -m 0644 "$ROOT/packaging/linux/terminus.desktop" "$APPDIR/usr/share/applications/terminus.desktop"
install -m 0644 "$ROOT/packaging/linux/icons/terminus-512.png" "$APPDIR/usr/share/icons/hicolor/512x512/apps/terminus.png"

# `--appimage-extract-and-run`: sandbox/CI biasanya TIDAK punya FUSE
# (AppImage normalnya butuh itu buat mount diri sendiri sebelum bisa
# dijalankan) — flag ini ekstrak dulu ke tmp lalu jalankan langsung
# tanpa mount, aman di lingkungan manapun termasuk container/sandbox.
"$LINUXDEPLOY" --appimage-extract-and-run \
    --appdir "$APPDIR" \
    --executable "$BIN" \
    --desktop-file "$ROOT/packaging/linux/terminus.desktop" \
    --icon-file "$ROOT/packaging/linux/icons/terminus-512.png"

mkdir -p "$ROOT/build"
"$APPIMAGETOOL" --appimage-extract-and-run "$APPDIR" "$ROOT/build/Terminus-x86_64.AppImage"

echo "OK -> $ROOT/build/Terminus-x86_64.AppImage"
