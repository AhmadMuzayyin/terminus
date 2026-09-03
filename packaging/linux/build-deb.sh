#!/usr/bin/env bash
# Bungkus binary rilis (SUDAH dikompilasi lewat `cargo build --release
# -p terminus-app` — script ini TIDAK menjalankan cargo build sendiri)
# jadi paket .deb pakai `dpkg-deb` (tersedia bawaan di banyak distro
# termasuk Fedora yang dipakai buat cross-packaging ini).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
VERSION="0.1.0"
ARCH="amd64"
PKG_DIR="$ROOT/build/deb/terminus_${VERSION}_${ARCH}"
BIN="$ROOT/target/release/terminus"

if [ ! -f "$BIN" ]; then
    echo "Binary rilis belum ada — jalankan dulu: cargo build --release -p terminus-app" >&2
    exit 1
fi

rm -rf "$PKG_DIR"
mkdir -p "$PKG_DIR/DEBIAN" \
         "$PKG_DIR/usr/bin" \
         "$PKG_DIR/usr/share/applications" \
         "$PKG_DIR/usr/share/icons/hicolor/512x512/apps"

install -m 0755 "$BIN" "$PKG_DIR/usr/bin/terminus"
install -m 0644 "$ROOT/packaging/linux/terminus.desktop" "$PKG_DIR/usr/share/applications/terminus.desktop"
install -m 0644 "$ROOT/packaging/linux/icons/terminus-512.png" "$PKG_DIR/usr/share/icons/hicolor/512x512/apps/terminus.png"

INSTALLED_SIZE=$(du -sk "$PKG_DIR" | cut -f1)

# Depends di bawah "best effort" dari `ldd target/release/terminus` di
# mesin Fedora (nama paket Debian-nya ditebak dari konvensi umum yang
# SANGAT stabil lintas rilis Debian/Ubuntu — tidak diverifikasi lewat
# apt-cache beneran karena dibangun dari Fedora, bukan Debian) — WAJIB
# dites instal di Debian/Ubuntu asli sebelum didistribusikan luas.
# `libxkbcommon0` ditambah jaga-jaga (winit butuh itu waktu runtime
# buat keyboard handling, tapi TIDAK muncul di `ldd` karena di-dlopen,
# bukan linked langsung).
cat > "$PKG_DIR/DEBIAN/control" <<EOF
Package: terminus
Version: ${VERSION}
Section: net
Priority: optional
Architecture: ${ARCH}
Installed-Size: ${INSTALLED_SIZE}
Maintainer: Terminus Contributors <noreply@example.com>
Homepage: https://github.com/yourname/terminus
Depends: libc6, libfontconfig1, libfreetype6, libharfbuzz0b, libglib2.0-0, libxml2, libpng16-16, libgraphite2-3, libxkbcommon0
Description: SSH/SFTP/Console manager native lintas platform
 Terminus adalah aplikasi manajemen koneksi SSH, SFTP, dan console
 (serial) native, ditulis 100% Rust dengan GUI Slint.
EOF

mkdir -p "$ROOT/build"
dpkg-deb --root-owner-group --build "$PKG_DIR" "$ROOT/build/terminus_${VERSION}_${ARCH}.deb"

echo "OK -> $ROOT/build/terminus_${VERSION}_${ARCH}.deb"
