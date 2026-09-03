#!/usr/bin/env bash
# HARUS dijalankan di macOS ASLI (dengan Xcode Command Line Tools
# terinstall, `xcode-select --install`) — TIDAK BISA cross-compile
# dari Linux/Windows. Slint pakai backend windowing native lewat
# `winit`, yang di macOS link langsung ke framework Cocoa/AppKit/
# CoreGraphics — framework-framework itu bagian dari macOS SDK, cuma
# ada kalau OS-nya beneran macOS (bukan sekadar target triple Rust;
# Apple tidak mendistribusikan SDK-nya buat dipasang di Linux secara
# resmi/legal, beda dari toolchain Linux/Windows yang bisa
# di-cross-compile bebas). Jalankan script ini di mesin Mac beneran,
# atau runner CI `macos-latest` (GitHub Actions, dst).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
VERSION="0.1.0"
APP_NAME="Terminus"
BUNDLE="$ROOT/build/${APP_NAME}.app"

if [ "$(uname)" != "Darwin" ]; then
    echo "Script ini cuma jalan di macOS (deteksi: $(uname)). Lihat komentar di atas." >&2
    exit 1
fi

echo "== cargo build --release =="
cargo build --release --manifest-path "$ROOT/Cargo.toml" -p terminus-app

BIN="$ROOT/target/release/terminus"
if [ ! -f "$BIN" ]; then
    echo "Build gagal, binary tidak ditemukan: $BIN" >&2
    exit 1
fi

echo "== Menyusun ${APP_NAME}.app =="
rm -rf "$BUNDLE"
mkdir -p "$BUNDLE/Contents/MacOS" "$BUNDLE/Contents/Resources"

cp "$BIN" "$BUNDLE/Contents/MacOS/terminus"
chmod +x "$BUNDLE/Contents/MacOS/terminus"

# Icon .icns — dikonversi DI SINI (bukan disimpan jadi file di repo)
# dari PNG proyek (`packaging/linux/icons/`) pakai `sips`/`iconutil`,
# dua-duanya bawaan macOS, TIDAK ada di Linux — makanya konversi ini
# baru bisa jalan pas script ini dieksekusi di Mac beneran.
ICONSET="$ROOT/build/terminus.iconset"
rm -rf "$ICONSET"
mkdir -p "$ICONSET"
for size in 16 32 128 256 512; do
    sips -z "$size" "$size" "$ROOT/packaging/linux/icons/terminus-512.png" --out "$ICONSET/icon_${size}x${size}.png" >/dev/null
    double=$((size * 2))
    sips -z "$double" "$double" "$ROOT/packaging/linux/icons/terminus-512.png" --out "$ICONSET/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$BUNDLE/Contents/Resources/terminus.icns"
rm -rf "$ICONSET"

cat > "$BUNDLE/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>${APP_NAME}</string>
    <key>CFBundleDisplayName</key>
    <string>${APP_NAME}</string>
    <key>CFBundleIdentifier</key>
    <string>com.terminus.app</string>
    <key>CFBundleVersion</key>
    <string>${VERSION}</string>
    <key>CFBundleShortVersionString</key>
    <string>${VERSION}</string>
    <key>CFBundleExecutable</key>
    <string>terminus</string>
    <key>CFBundleIconFile</key>
    <string>terminus.icns</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>LSMinimumSystemVersion</key>
    <string>11.0</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
EOF

echo "OK -> $BUNDLE"

echo "== Menyusun .dmg =="
DMG="$ROOT/build/${APP_NAME}-${VERSION}.dmg"
rm -f "$DMG"
STAGING="$ROOT/build/dmg-staging"
rm -rf "$STAGING"
mkdir -p "$STAGING"
cp -R "$BUNDLE" "$STAGING/"
ln -s /Applications "$STAGING/Applications"
hdiutil create -volname "$APP_NAME" -srcfolder "$STAGING" -ov -format UDZO "$DMG"
rm -rf "$STAGING"

echo "OK -> $DMG"
echo
echo "CATATAN: binary belum di-codesign/notarize. Di mesin Mac lain,"
echo "Gatekeeper akan menandainya \"unidentified developer\" (bisa"
echo "tetap dibuka lewat klik-kanan -> Open). Untuk distribusi publik"
echo "yang mulus (tanpa peringatan itu), perlu Apple Developer ID +"
echo "codesign + notarize — di luar scope script ini."
