#!/usr/bin/env bash
# Bungkus binary rilis (SUDAH dikompilasi lewat `cargo build --release
# -p terminus-app`) jadi paket .rpm pakai `rpmbuild` — spec-nya
# "binary-only" (lihat komentar panjang di `terminus.spec`), tidak
# compile ulang di dalam sandbox rpmbuild.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BIN="$ROOT/target/release/terminus"
TOPDIR="$ROOT/build/rpmbuild"

if [ ! -f "$BIN" ]; then
    echo "Binary rilis belum ada — jalankan dulu: cargo build --release -p terminus-app" >&2
    exit 1
fi

rm -rf "$TOPDIR"
mkdir -p "$TOPDIR"/{SOURCES,SPECS,BUILD,RPMS,SRPMS,BUILDROOT}

cp "$BIN" "$TOPDIR/SOURCES/terminus"
cp "$ROOT/packaging/linux/terminus.desktop" "$TOPDIR/SOURCES/terminus.desktop"
cp "$ROOT/packaging/linux/icons/terminus-512.png" "$TOPDIR/SOURCES/terminus-512.png"
cp "$ROOT/packaging/linux/terminus.spec" "$TOPDIR/SPECS/terminus.spec"

rpmbuild --define "_topdir $TOPDIR" -bb "$TOPDIR/SPECS/terminus.spec"

mkdir -p "$ROOT/build"
find "$TOPDIR/RPMS" -name "*.rpm" -exec cp {} "$ROOT/build/" \;

echo "OK -> lihat $ROOT/build/*.rpm"
