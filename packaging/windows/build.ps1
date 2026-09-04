# HARUS dijalankan di Windows asli (PowerShell) — tidak bisa
# cross-compile dari Linux/macOS, sama alasannya dengan macOS: link
# ke toolchain MSVC yang cuma ada di Windows. Windows TIDAK butuh
# "packaging" berlapis ala Linux (.deb/.rpm/AppImage) — satu file
# .exe hasil `cargo build --release` sudah bisa langsung dijalankan/
# didistribusikan apa adanya, jadi script ini cuma build + rapikan
# output ke `build\` + bikin .zip biar gampang dibagikan.
$ErrorActionPreference = "Stop"

$Root = Resolve-Path "$PSScriptRoot\..\.."
$Version = "0.1.0"

Write-Host "== cargo build --release =="
cargo build --release --manifest-path "$Root\Cargo.toml" -p terminus-app

$Bin = "$Root\target\release\terminus.exe"
if (-not (Test-Path $Bin)) {
    Write-Error "Build gagal, binary tidak ditemukan: $Bin"
    exit 1
}

New-Item -ItemType Directory -Force -Path "$Root\build" | Out-Null
Copy-Item $Bin "$Root\build\terminus.exe" -Force

$Zip = "$Root\build\terminus-$Version-windows-x86_64.zip"
if (Test-Path $Zip) { Remove-Item $Zip }
Compress-Archive -Path "$Root\build\terminus.exe" -DestinationPath $Zip

Write-Host "OK -> $Root\build\terminus.exe"
Write-Host "OK -> $Zip"
Write-Host ""
Write-Host "CATATAN: binary belum di-codesign. Windows SmartScreen"
Write-Host "kemungkinan menandainya 'unrecognized app' di mesin lain"
Write-Host "(tetap bisa dijalankan lewat 'More info' -> 'Run anyway')."
Write-Host "Installer MSI/NSIS + code-signing certificate belum dibuat"
Write-Host "-- di luar scope script ini."
