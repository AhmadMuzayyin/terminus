# Build & Rilis

Halaman ini menjelaskan cara membangun Terminus dari source menjadi
file yang bisa dijalankan atau dibagikan: executable desktop (Linux,
macOS, Windows), paket siap-instal, dan APK aplikasi Android.

Semua hasil paket masuk ke folder `build/` di root proyek; binary
mentah ada di `target/release/`. Keduanya tidak ikut ke-commit.

> [!IMPORTANT]
> Build desktop **tidak bisa cross-compile antar OS**. Executable macOS
> harus dibangun di Mac, executable Windows harus dibangun di Windows.
> Kalau perlu ketiganya sekaligus, pakai runner CI per OS
> (`ubuntu-latest`, `macos-latest`, `windows-latest`).

## Prasyarat

| Target | Yang dibutuhkan |
| --- | --- |
| Desktop (semua OS) | Rust stable ≥ 1.85 + library native Slint — lihat **[Instalasi](#getting-started)** |
| Paket Linux | `dpkg-deb` (untuk `.deb`), `rpmbuild` (untuk `.rpm`), `curl` + internet (untuk AppImage) |
| macOS | Xcode Command Line Tools (`xcode-select --install`) |
| Windows | Visual Studio Build Tools, workload "Desktop development with C++" |
| Android | JDK 17 atau lebih baru + Android SDK (platform 37) — lihat [Aplikasi Android](#packaging:aplikasi-android-apk) |

## Executable Desktop

Cara paling cepat: satu perintah `cargo` menghasilkan satu file
executable yang langsung bisa dijalankan, tanpa installer.

```bash tab="Linux"
cargo build --release -p terminus-app
./target/release/terminus
```

```bash tab="macOS"
cargo build --release -p terminus-app
./target/release/terminus
```

```powershell tab="Windows"
cargo build --release -p terminus-app
.\target\release\terminus.exe
```

Build pertama memakan waktu beberapa menit karena seluruh dependency
dikompilasi dengan optimasi penuh (LTO, `codegen-units = 1`); build
berikutnya jauh lebih cepat. Binary rilis sudah di-*strip* (tanpa
simbol debug), jadi ukurannya kecil.

> [!NOTE]
> Executable ini tetap butuh library sistem yang sama dengan saat
> development (mis. `fontconfig`, `libxkbcommon`, `wayland` di Linux).
> Untuk dibagikan ke mesin lain, lebih aman pakai paket di bawah —
> terutama AppImage, yang membawa semua library-nya sendiri.

## Paket Siap-Distribusi

Script di
[`packaging/`](https://github.com/AhmadMuzayyin/terminus/tree/main/packaging)
membungkus binary rilis menjadi format yang lazim di tiap OS.

### Linux (`.deb`, `.rpm`, AppImage)

```bash
# Semua sekaligus (build release dijalankan sekali di dalamnya):
./packaging/linux/build-all.sh

# Atau satu-satu — jalankan `cargo build --release -p terminus-app` dulu:
./packaging/linux/build-deb.sh
./packaging/linux/build-rpm.sh
./packaging/linux/build-appimage.sh
```

Hasil:

- `build/terminus_0.1.0_amd64.deb` — Debian/Ubuntu: `sudo apt install ./build/terminus_0.1.0_amd64.deb`
- `build/terminus-0.1.0-1.*.rpm` — Fedora/openSUSE: `sudo dnf install ./build/terminus-0.1.0-1.*.rpm`
- `build/Terminus-x86_64.AppImage` — distro apa pun: `chmod +x` lalu jalankan langsung

Catatan:

- `.deb`/`.rpm` disusun manual lewat `dpkg-deb`/`rpmbuild` (bukan
  `cargo-deb`/`cargo-generate-rpm`). Daftar dependency-nya "best
  effort" dari `ldd` binary rilis, jadi sebaiknya dites instal dulu di
  Debian/Ubuntu/Fedora asli sebelum didistribusikan luas.
- AppImage disusun lewat `linuxdeploy` + `appimagetool` (diunduh
  otomatis sekali, di-cache di `packaging/linux/tools/`), dan
  membundel semua shared library supaya portable.

### macOS (`.app`, `.dmg`)

Jalankan di Mac asli (Apple Silicon maupun Intel — hasilnya mengikuti
arsitektur mesin yang dipakai):

```bash
./packaging/macos/build.sh
```

Hasil: `build/Terminus.app` dan `build/Terminus-0.1.0.dmg`. Ikon
`.icns` dibuat otomatis dari PNG proyek memakai `sips`/`iconutil`
bawaan macOS.

> [!WARNING]
> Binary belum di-codesign/notarize. Di Mac lain, Gatekeeper
> menandainya "unidentified developer" — buka lewat klik-kanan → Open.
> Distribusi publik tanpa peringatan itu butuh Apple Developer ID.

### Windows (`.exe`, `.zip`)

Jalankan di Windows asli (PowerShell):

```powershell
.\packaging\windows\build.ps1
```

Hasil: `build\terminus.exe` dan
`build\terminus-0.1.0-windows-x86_64.zip`. Windows tidak butuh format
paket berlapis — satu `.exe` sudah bisa langsung dijalankan atau
dibagikan.

> [!WARNING]
> Binary belum di-codesign. SmartScreen mungkin menandainya
> "unrecognized app" di mesin lain (tetap bisa dijalankan lewat "More
> info" → "Run anyway"). Installer MSI/NSIS belum tersedia.

### Ikon

Ikon aplikasi ada di `packaging/linux/icons/` (PNG berbagai ukuran)
dan dipakai ulang oleh ketiga platform.

## Aplikasi Android (APK)

Project Android ada di folder `mobile/` dan dibangun dengan Gradle
wrapper bawaan — tidak perlu memasang Gradle terpisah. Semua perintah
di bawah dijalankan dari dalam folder `mobile/` (`cd mobile`).

### Menyiapkan SDK

Gradle perlu tahu lokasi Android SDK. Cara termudah: pasang Android
Studio sekali, lalu arahkan lewat salah satu dari ini:

```bash tab="Linux / macOS"
export ANDROID_HOME="$HOME/Android/Sdk"     # macOS: $HOME/Library/Android/sdk
```

```powershell tab="Windows"
$env:ANDROID_HOME = "$env:LOCALAPPDATA\Android\Sdk"
```

Atau buat file `mobile/local.properties` berisi
`sdk.dir=/path/ke/Android/Sdk` (file ini tidak ikut ke-commit). Paket
SDK yang kurang (platform 37, build-tools) diunduh otomatis oleh
Gradle saat build pertama.

### APK debug (untuk uji coba)

```bash tab="Linux / macOS"
./gradlew assembleDebug
```

```powershell tab="Windows"
.\gradlew.bat assembleDebug
```

Hasil: `mobile/app/build/outputs/apk/debug/app-debug.apk` — bisa
langsung dipasang ke HP, tapi lebih besar dan lebih lambat dari APK
rilis.

### APK rilis

APK rilis dioptimasi R8 (kode tak terpakai dibuang, ukuran jauh lebih
kecil) dan ditandatangani **kunci rilis milikmu sendiri**.

**1. Buat keystore (sekali saja)** — dari folder `mobile/`:

```bash
keytool -genkeypair -v -keystore ../terminus-release.jks \
    -alias terminus -keyalg RSA -keysize 4096 -validity 10000
```

**2. Buat `mobile/keystore.properties`:**

```properties
storeFile=../terminus-release.jks
storePassword=password-keystore
keyAlias=terminus
keyPassword=password-key
```

`storeFile` relatif terhadap folder `mobile/`.

> [!CAUTION]
> `keystore.properties` dan file `.jks` adalah **rahasia** — keduanya
> sudah ada di `.gitignore`, jangan pernah di-commit. Simpan backup
> keystore di tempat aman: kalau hilang, update aplikasi di Play Store
> tidak bisa dirilis lagi.

**3. Build:**

```bash tab="Linux / macOS"
./gradlew assembleRelease     # APK, untuk dipasang langsung
./gradlew bundleRelease       # AAB, untuk diunggah ke Play Store
```

```powershell tab="Windows"
.\gradlew.bat assembleRelease
.\gradlew.bat bundleRelease
```

Hasil:

- APK: `mobile/app/build/outputs/apk/release/app-release.apk`
- AAB: `mobile/app/build/outputs/bundle/release/app-release.aab`

> [!NOTE]
> Kalau `keystore.properties` tidak ada, APK rilis tetap terbentuk
> tetapi ditandatangani kunci **debug** — cukup untuk uji di HP
> sendiri, tidak bisa diunggah ke Play Store.

Sebelum merilis versi baru, naikkan `versionCode` (angka, wajib
selalu naik) dan `versionName` di `mobile/app/build.gradle.kts`.

> [!WARNING]
> R8 bisa merusak library yang memakai reflection, dan itu baru
> ketahuan saat aplikasi berjalan. Setiap kali dependency atau aturan
> R8 (`app/proguard-rules.pro`) berubah, uji ulang APK rilis secara
> penuh: login, SSH, dan SFTP.

### Memasang ke HP

Aktifkan **Developer options → USB debugging** di HP, sambungkan
lewat USB, lalu:

```console
$ adb devices
List of devices attached
R58N12ABCDE     device
$ adb install -r app/build/outputs/apk/release/app-release.apk
Performing Streamed Install
Success
```

Atau salin file APK ke HP dan buka dari file manager (izinkan
"Install unknown apps" untuk aplikasi file manager tersebut).

Cara memakai aplikasinya ada di **[Aplikasi Mobile](#mobile)**.

## Ringkasan Lokasi Hasil Build

| Perintah | Hasil |
| --- | --- |
| `cargo build --release -p terminus-app` | `target/release/terminus` (`.exe` di Windows) |
| `packaging/linux/build-all.sh` | `build/*.deb`, `build/*.rpm`, `build/Terminus-x86_64.AppImage` |
| `packaging/macos/build.sh` | `build/Terminus.app`, `build/Terminus-0.1.0.dmg` |
| `packaging\windows\build.ps1` | `build\terminus.exe`, `build\terminus-0.1.0-windows-x86_64.zip` |
| `./gradlew assembleDebug` | `mobile/app/build/outputs/apk/debug/app-debug.apk` |
| `./gradlew assembleRelease` | `mobile/app/build/outputs/apk/release/app-release.apk` |
| `./gradlew bundleRelease` | `mobile/app/build/outputs/bundle/release/app-release.aab` |
