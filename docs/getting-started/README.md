# Instalasi

## 1. Rust Toolchain

Butuh Rust edisi 2021 terbaru (stable, minimal 1.85 — beberapa
dependency transitif mensyaratkan versi itu).

```bash
# Opsi A — rustup (disarankan, portable di Linux/macOS/Windows)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Opsi B — paket distro (contoh Fedora)
sudo dnf install rust cargo
```

Verifikasi: `rustc --version`.

## 2. Library Native untuk Slint (Rendering GUI)

Slint butuh toolchain C (linking) + library windowing/font sistem —
beda-beda per OS.

### Linux

```bash
# Fedora
sudo dnf install gcc gcc-c++ pkgconf-pkg-config \
    fontconfig-devel libxkbcommon-devel wayland-devel libX11-devel

# Debian/Ubuntu
sudo apt install build-essential pkg-config \
    libfontconfig1-dev libxkbcommon-dev libwayland-dev libx11-dev

# Arch
sudo pacman -S base-devel pkgconf fontconfig libxkbcommon wayland libx11
```

Slint otomatis pilih backend Wayland kalau `$WAYLAND_DISPLAY` ada,
fallback ke X11 kalau tidak — aman pasang dua-duanya library di atas.

### macOS

Cukup **Xcode Command Line Tools**:

```bash
xcode-select --install
```

Slint pakai backend native macOS (AppKit/Metal lewat `winit`), tidak
butuh paket tambahan lain. Berjalan native di Apple Silicon maupun
Intel — Cargo otomatis compile sesuai arsitektur mesin yang dipakai.

### Windows

1. Pasang **Visual Studio Build Tools** (bukan Visual Studio penuh)
   dari [visualstudio.microsoft.com/downloads](https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022)
   — waktu instalasi, **wajib centang workload "Desktop development
   with C++"** (menyertakan MSVC linker yang dibutuhkan Rust target
   `x86_64-pc-windows-msvc`).
2. Pasang Rust lewat [rustup-init.exe](https://rustup.rs) — otomatis
   mendeteksi & memakai toolchain MSVC yang baru dipasang.

```powershell
rustc --version
cargo --version
```

Slint pakai backend native Win32/Direct3D, **tidak butuh WebView2**
atau runtime GUI tambahan apa pun — beda dari framework berbasis web
view (Tauri/Electron).

> `rusqlite` (dipakai vault) pakai feature `bundled` — SQLite
> di-compile dari source lewat `cc`, jadi **tidak perlu** install
> `sqlite-devel`/`libsqlite3-dev` terpisah di platform manapun
> (termasuk Windows — `cc` otomatis pakai toolchain MSVC di atas).

## 3. Font (Opsional, Kosmetik)

Desain merujuk **Hanken Grotesk** (headline), **Inter** (body), dan
**JetBrains Mono** (data teknis & terminal). Kalau tidak terinstall,
Slint otomatis fallback ke font default sistem tanpa error — tampilan
tetap rapi.

```bash
# Fedora — Inter & JetBrains Mono tersedia di repo resmi
sudo dnf install rsms-inter-fonts jetbrains-mono-fonts-all
```

Hanken Grotesk tidak dipaketkan distro manapun — download manual dari
[Google Fonts](https://fonts.google.com/specimen/Hanken+Grotesk) kalau
mau match persis; headline tetap terbaca jelas tanpa itu.

## Selanjutnya

Setelah semua terpasang, lanjut ke **[Menjalankan Aplikasi](../usage/)**.
