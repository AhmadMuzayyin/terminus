Name:           terminus
Version:        0.1.0
Release:        1%{?dist}
Summary:        SSH/SFTP/Console manager native lintas platform

License:        GPL-3.0-or-later
URL:             https://github.com/yourname/terminus
# Spec ini SENGAJA "binary-only" (bukan build-from-source di dalam
# rpmbuild) — binary rilis-nya sudah dikompilasi duluan lewat
# `cargo build --release` (lihat `packaging/linux/build-rpm.sh`),
# rpmbuild di sini cuma membungkusnya jadi paket .rpm. Ini pola yang
# sama dipakai banyak proyek yang toolchain build aslinya (di sini:
# Cargo workspace) tidak cocok/tidak perlu diulang di dalam sandbox
# rpmbuild sendiri.
BuildArch:      x86_64
# Fedora tidak generate dependency versi Debian secara otomatis buat
# proyek non-Fedora — daftar di bawah "best effort" dari `ldd` binary
# rilis-nya (lihat komentar sama persis di `build-deb.sh`), BUKAN
# hasil `rpmbuild --define auto-reqprov` (kita matikan itu di bawah,
# `AutoReqProv: no`, biar tidak coba nebak dependency dari isi binary
# yang statically-linked sebagiannya, misleading).
AutoReqProv:    no
Requires:       fontconfig, libxkbcommon

%description
Terminus — manajer koneksi SSH, SFTP, dan console (serial) native,
ditulis 100%% Rust dengan GUI Slint. Lihat README proyek untuk detail
lengkap.

%prep
# Tidak ada apa-apa buat di-"prep" — sumber binary sudah jadi, lihat
# komentar %files & catatan "binary-only" di atas.

%build
# Tidak ada build di sini juga, sama alasannya.

%install
rm -rf %{buildroot}
mkdir -p %{buildroot}/usr/bin
mkdir -p %{buildroot}/usr/share/applications
mkdir -p %{buildroot}/usr/share/icons/hicolor/512x512/apps
install -m 0755 %{_sourcedir}/terminus %{buildroot}/usr/bin/terminus
install -m 0644 %{_sourcedir}/terminus.desktop %{buildroot}/usr/share/applications/terminus.desktop
install -m 0644 %{_sourcedir}/terminus-512.png %{buildroot}/usr/share/icons/hicolor/512x512/apps/terminus.png

%files
/usr/bin/terminus
/usr/share/applications/terminus.desktop
/usr/share/icons/hicolor/512x512/apps/terminus.png

%changelog
* Thu Sep 03 2026 Terminus Contributors <noreply@example.com> - 0.1.0-1
- Rilis awal packaging RPM.
