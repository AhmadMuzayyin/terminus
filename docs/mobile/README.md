# Aplikasi Mobile (Android)

Terminus Mobile adalah aplikasi Android untuk **server Terminus
Self-hosted**. Host, grup, identity, dan password yang kamu lihat di HP
**sama persis** dengan yang ada di desktop app mode Self-hosted — semua
dibaca & disimpan langsung ke server, tidak ada salinan terpisah di HP.

Yang bisa dilakukan dari HP:

- Kelola **host, grup, dan identity** (tambah, edit, duplikat, hapus).
- Buka **terminal SSH** — beberapa sesi sekaligus, tetap hidup waktu
  aplikasi di background.
- **SFTP**: jelajah folder server, unduh ke HP, unggah dari HP.
- Atur **profil akun**, **kunci aplikasi** (sidik jari / kunci layar),
  **ukuran font**, dan **tema terminal**.

> Aplikasi mobile **hanya** bekerja dengan server Self-hosted. Vault
> lokal (mode Local di desktop) tidak tersedia di HP.

## Yang Dibutuhkan

- HP **Android 10** atau lebih baru.
- **Server Terminus Self-hosted** yang sudah berjalan, beserta
  alamatnya (contoh: `http://192.168.1.10:4000` atau
  `https://vault.contoh.id`).
- **Akun** di server itu (email + password). Server yang baru dipasang
  dan belum punya akun sama sekali bisa langsung didaftarkan admin
  pertamanya dari HP (lihat bagian *Masuk ke Server* di bawah).
- HP harus bisa menjangkau **server Terminus** dan juga **host SSH**
  yang mau dibuka (satu jaringan/VPN yang sama).

## Memasang Aplikasi

Saat ini aplikasi belum ada di Play Store — dipasang dari file **APK**.

### Build APK dari source

Butuh [Android SDK](https://developer.android.com/studio) (cukup
"command-line tools"). JDK untuk compile diunduh otomatis oleh Gradle.

```bash
cd mobile
echo "sdk.dir=$HOME/Android/Sdk" > local.properties   # lokasi Android SDK
./gradlew assembleRelease
# hasil: mobile/app/build/outputs/apk/release/app-release.apk
```

Tanpa kunci rilis, APK ditandatangani dengan kunci debug — cukup untuk
dipakai sendiri. Untuk distribusi (mis. Play Store), siapkan kunci
rilis sendiri: lihat bagian "Build Rilis & Penandatanganan" di
[`mobile/DESIGN.md`](https://github.com/AhmadMuzayyin/terminus/blob/main/mobile/DESIGN.md).

### Pasang ke HP

- **Lewat kabel USB** (Opsi Pengembang → *USB debugging* aktif):
  `adb install mobile/app/build/outputs/apk/release/app-release.apk`
- **Tanpa kabel**: kirim file APK ke HP, buka, lalu izinkan
  *"Instal aplikasi tidak dikenal"* untuk aplikasi pembuka file itu.

Nama paket aplikasi: `com.ustdev.terminus`.

## Masuk ke Server

1. Isi **Server URL** — alamat server Terminus (boleh tanpa `http://`,
   akan dilengkapi otomatis).
2. Isi **Email** dan **Password**, tekan **Masuk**.

Hal yang perlu diketahui:

- **Server baru?** Tekan *"Server baru? Daftar admin pertama"* untuk
  membuat akun admin (Nama Lengkap, Email, Password minimal 8
  karakter). Ini **hanya bisa** dilakukan sekali, selama server belum
  punya akun sama sekali.
- Alamat `http://` (bukan `https://`) tetap boleh — cocok untuk server di
  jaringan rumah/kantor — tapi aplikasi menampilkan peringatan karena
  koneksinya **tidak terenkripsi**. Pakai `https://` kalau server bisa
  diakses dari internet.
- Setelah berhasil masuk, **membuka aplikasi berikutnya langsung masuk
  otomatis** ("Masuk otomatis…") sampai kamu Logout.
- Kalau muncul **"Sesi berakhir, silakan masuk lagi."**, sesi login di
  HP ini sudah dicabut — biasanya karena password akun diganti dari
  perangkat lain. Masuk lagi dengan password yang baru.

## Tampilan Utama

Setelah masuk, ada empat tab di bawah layar:

| Tab | Isi |
|---|---|
| **Hosts** | Daftar grup & host, pencarian, buka terminal |
| **SFTP** | Jelajah & transfer file di server |
| **Identities** | Username + password yang bisa dipakai ulang untuk mengisi host |
| **Akun** | Profil, pengaturan, logout |

## Hosts

- Di halaman utama tampil **grup** (ketuk untuk masuk) dan **host tanpa
  grup**. Di dalam grup, tombol **←** atau tombol *Back* HP kembali ke
  daftar utama.
- **Cari** host lewat label, IP/hostname, username, atau nama grup.
  Beberapa kata sekaligus boleh (mis. `prod web`).
- **Tarik layar ke bawah** untuk memuat ulang dari server (misalnya
  setelah ada perubahan dari desktop).
- Tombol **+**:
  - di halaman utama → pilih **Host baru** atau **Grup baru**;
  - di dalam grup → langsung **Host baru** di grup itu.
- **Menu host/grup**: ketuk ikon **⋮** di kanan, atau **tahan** barisnya.
  - Host: **Edit**, **Duplikat** (password ikut tersalin), **Hapus**.
  - Grup: **Ganti nama**, **Hapus**.

> **Menghapus grup ikut menghapus semua host di dalamnya** beserta
> password tersimpannya — sama seperti di desktop. Aplikasi menampilkan
> jumlah host yang ikut terhapus sebelum kamu mengonfirmasi.

### Form host

| Isian | Keterangan |
|---|---|
| Isi dari identity | Pilih identity → username & password terisi otomatis |
| Label | Nama yang tampil di daftar. Kosong = pakai Host/IP |
| Host / IP | Alamat server SSH |
| Port | Biasanya `22` |
| Username | Boleh kosong — akan ditanyakan waktu connect |
| Grup | Pilih grup, atau *Tanpa grup* |
| Password | Boleh kosong. Saat edit: **kosongkan kalau tidak diubah** |

Host tanpa password ditandai *"tanpa password"* di daftar.

## Terminal SSH

**Ketuk host** di tab Hosts untuk membuka terminal.

- Kalau host belum punya username/password tersimpan, aplikasi
  menanyakannya dulu. Centang **Simpan ke server** supaya lain kali
  tidak ditanya lagi (tersimpan untuk semua perangkat, termasuk desktop).
- Pertama kali membuka sesi terminal, Android meminta **izin
  notifikasi** — izinkan supaya notifikasi "sesi SSH aktif" terlihat.

### Pertama kali terhubung ke sebuah server

Muncul dialog **"Host belum dikenal"** berisi **sidik jari** (fingerprint)
kunci server, contoh `SHA256:VqrdF8Gd…`. Cocokkan dengan sidik jari
asli server sebelum menekan **Percayai & sambungkan**. Cara melihatnya
di server (lewat konsol/akses lain yang kamu percaya):

```bash
ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub
```

Setelah dipercaya, HP mengingat kunci itu dan tidak bertanya lagi.

### Peringatan "HOST KEY … BERUBAH"

Kunci server **berbeda** dari yang dulu dipercaya, dan koneksi
**diblokir**. Penyebabnya bisa wajar (server diinstal ulang, IP dipakai
server lain) atau berbahaya (ada pihak yang menyadap koneksi).

- **Yakin** servernya memang berganti → **Hapus kunci lama** →
  **Sambung ulang** → periksa sidik jari baru seperti di atas.
- **Tidak yakin** → jangan dihapus; tanyakan admin server dulu.

### Mengetik & tombol ekstra

- **Ketuk layar terminal** untuk memunculkan keyboard.
- Baris tombol di atas keyboard: `ESC`, `CTRL`, `ALT`, `TAB`, panah
  `← ↑ ↓ →`, `-`, `/`, `|`, `~`.
- **CTRL** dan **ALT** bersifat *tempel*: tekan sekali (tombolnya
  menyala), lalu tekan huruf. Contoh menghentikan perintah: **CTRL**
  lalu **c**.
- **Salin**: tahan teks di terminal → geser pegangan seleksi →
  **Salin**.
- **Tempel**: menu **⋮** kanan atas → **Tempel**.
- **Ukuran huruf**: cubit (*pinch*) layar terminal untuk memperbesar /
  memperkecil. Ukuran ini diingat.
- Geser layar ke atas/bawah untuk melihat riwayat output.
- Ukuran terminal menyesuaikan layar & rotasi otomatis.
- Di mode **landscape dengan keyboard terbuka**, bar sesi di atas
  disembunyikan supaya terminal tetap kelihatan.

### Beberapa sesi sekaligus

- Buka host lain dari tab Hosts — sesinya muncul sebagai tab baru di
  bar atas terminal. Ketuk tab untuk berpindah, **✕** untuk menutup.
- Tombol **←** kiri atas kembali ke daftar host **tanpa** menutup sesi.
  Selama ada sesi terbuka, muncul bar *"N sesi terminal aktif — ketuk
  untuk membuka"* di atas navigasi bawah.
- Sesi **tetap tersambung waktu aplikasi di background** (ditandai
  notifikasi *"N sesi SSH aktif"*).
- Sesi berakhir (perintah `exit`, jaringan putus) → layar & riwayatnya
  tetap ada; tekan **Sambung ulang** untuk melanjutkan di tab yang sama.

## SFTP

1. Buka tab **SFTP** → pilih host (ada kolom pencarian).
2. Isi folder home tampil. **Ketuk folder** untuk masuk, `..` atau
   tombol *Back* untuk naik.

| Aksi | Caranya |
|---|---|
| **Unduh file** | Ketuk file (atau ⋮ → Unduh) → pilih lokasi & nama di HP → Simpan |
| **Unggah file** | Tombol **+** → **Unggah file** → pilih file di HP |
| **Folder baru** | Tombol **+** → **Folder baru** |
| **Ganti nama / Hapus** | Ikon **⋮** atau tahan baris |
| **Muat ulang** | Tarik layar ke bawah |
| **Putuskan** | Ikon **✕** kanan atas |

- Unggah ke nama yang **sudah ada** → aplikasi bertanya **"Timpa file?"**.
- Menghapus **folder** ikut menghapus **seluruh isinya** (ada peringatan).
  Menghapus *tautan* (symlink) hanya menghapus tautannya, bukan isi
  tujuannya.
- Transfer menampilkan progres di bawah layar dan bisa **dibatalkan**.
  File setengah jadi otomatis dihapus. Satu transfer berjalan dalam satu
  waktu.
- Aplikasi **tidak** meminta izin akses penyimpanan — kamu yang memilih
  file/lokasi lewat pemilih file bawaan Android.
- Koneksi SFTP tetap hidup waktu pindah tab atau aplikasi di background.

## Identities

Identity = pasangan **username + password** yang bisa dipakai berulang
untuk mengisi form host (*"Isi dari identity"*).

- Tombol **+** untuk menambah (label, username, password).
- Ketuk identity untuk **edit** — password boleh dikosongkan kalau tidak
  diubah.
- Menghapus identity **tidak** mengubah host yang dulu diisi darinya.

## Akun & Pengaturan

### Profil & keamanan

- **Nama lengkap** dan **email** bisa diubah. Mengganti email wajib
  mengisi **password saat ini**.
- **Ganti password** (password baru minimal 8 karakter): **semua
  perangkat lain** (desktop, HP lain) langsung logout dan perlu masuk
  dengan password baru. HP yang dipakai untuk mengganti tetap masuk.

### Pengaturan

| Pengaturan | Keterangan |
|---|---|
| **Kunci aplikasi** | Minta sidik jari/wajah atau kunci layar HP (PIN/pola/sandi) waktu aplikasi dibuka dan setelah **5 menit** di background. Menyalakan & mematikannya perlu konfirmasi yang sama. HP harus sudah punya kunci layar. |
| **Ukuran font terminal** | Geser slider (ada pratinjau), atau cubit layar terminal |
| **Tema terminal** | 6 tema, sama dengan desktop: Terminus Dark, Terminus Light, Midnight Blue, Solar Flare, Mono Green, Mono Amber |

Tema yang dipilih di sini dipakai untuk host yang **belum punya tema
sendiri**. Host yang temanya sudah diatur dari desktop tetap memakai
tema itu.

Selama aplikasi terkunci, sesi SSH & SFTP **tetap berjalan** di
belakang — hanya tampilannya yang ditutup.

### Logout

Tab **Akun** → **Logout**. Semua sesi SSH & SFTP ditutup, sesi login di
server dicabut, dan membuka aplikasi berikutnya perlu masuk lagi (alamat
server tetap terisi).

## Keamanan & Privasi

Yang disimpan **di HP**:

- Token login — dienkripsi dengan kunci perangkat keras HP (Android
  Keystore) dan tidak bisa dipindah ke HP lain.
- Alamat server, sidik jari server SSH yang sudah kamu percaya, dan
  pengaturan aplikasi.

Yang **tidak** disimpan di HP:

- Daftar host dan password. Password host diambil dari server **hanya**
  saat connect, lalu dibuang dari memori.

Data aplikasi tidak ikut dicadangkan ke backup cloud Android.

## Pemecahan Masalah

**"Tidak bisa terhubung ke server"**
- Pastikan Server URL benar (termasuk port, mis. `:4000`) dan HP satu
  jaringan/VPN dengan server.
- Coba buka alamat yang sama di browser HP.
- Firewall server harus mengizinkan port tersebut.

**Sesi SSH putus waktu aplikasi di background**
- Pastikan notifikasi *"sesi SSH aktif"* muncul (izin notifikasi aktif).
- Beberapa merek HP (Xiaomi/Redmi/POCO, Oppo, Vivo, Samsung, dll.)
  mematikan aplikasi di background secara agresif. Buka *Setelan →
  Aplikasi → Terminus → Baterai* dan pilih **Tanpa batasan** (nama
  menunya berbeda-beda tiap merek).

**"Username atau password SSH salah"**
- Tekan **Masukkan password** untuk mencoba lagi. Centang *Simpan ke
  server* kalau ingin password yang benar tersimpan.

**Kunci aplikasi tidak bisa diaktifkan**
- Atur dulu kunci layar (PIN/pola/sandi) atau sidik jari di Setelan HP.

**Lupa password akun**
- Tidak bisa direset dari HP. Hubungi admin server Terminus kamu.

## Keterbatasan Versi Ini

- Hanya **Android** (belum ada versi iOS).
- Login SSH hanya dengan **password** (belum mendukung *private key*).
- Belum ada port forwarding, Telnet/Mosh, atau console serial (USB).
- Akun dengan lebih dari satu vault: yang dipakai vault pertama.
- Import/Export XML/JSON dilakukan dari desktop (datanya sama di server).
