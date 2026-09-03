#!/usr/bin/env bash
# Jalankan ketiga packaging Linux sekaligus (.deb, .rpm, AppImage) —
# hasilnya masuk ke `build/` di root proyek. Build release Rust-nya
# SENGAJA cuma sekali di sini (dipakai ulang oleh ketiga script,
# masing-masing TIDAK compile ulang sendiri-sendiri).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

echo "== 1/4: cargo build --release =="
cargo build --release --manifest-path "$ROOT/Cargo.toml" -p terminus-app

echo "== 2/4: .deb =="
"$ROOT/packaging/linux/build-deb.sh"

echo "== 3/4: .rpm =="
"$ROOT/packaging/linux/build-rpm.sh"

echo "== 4/4: AppImage =="
"$ROOT/packaging/linux/build-appimage.sh"

echo
echo "Semua paket Linux ada di: $ROOT/build/"
ls -la "$ROOT/build"/*.deb "$ROOT/build"/*.rpm "$ROOT/build"/*.AppImage 2>/dev/null || true
