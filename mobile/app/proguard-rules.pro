# Aturan R8 untuk build RILIS (mobile/DESIGN.md bagian 11). Tiap aturan
# di sini ada alasannya — jangan tambah "-keep class **" asal lolos build.

# BouncyCastle: provider JCA memuat kelas algoritma lewat NAMA kelas di string
# (reflection) — dibuang/diganti nama = Ed25519/curve25519 SSH gagal saat runtime.
-keep class org.bouncycastle.** { *; }
-dontwarn org.bouncycastle.**

# sshj: daftar algoritma (kex, cipher, MAC, host key) dibuat lewat factory +
# nama; beberapa kelas dimuat via reflection/ServiceLoader.
-keep class net.schmizz.sshj.** { *; }
-keep class com.hierynomus.sshj.** { *; }
-dontwarn net.schmizz.sshj.**
-dontwarn com.hierynomus.sshj.**

# sshj memakai slf4j-api tanpa implementasi (log dibuang) — kelas binding opsional.
-dontwarn org.slf4j.**
