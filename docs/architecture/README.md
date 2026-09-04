# Arsitektur

## Tumpukan Teknologi

| Lapisan | Pustaka | Kegunaan |
| --- | --- | --- |
| GUI | [Slint](https://slint.dev) 1.17 | UI declarative, native (bukan web view) |
| Async runtime | [Tokio](https://tokio.rs) | Semua I/O jaringan/serial/vault jalan di sini |
| SSH | [`russh`](https://github.com/Eugeny/russh) | Klien SSH murni Rust |
| SFTP | [`russh-sftp`](https://docs.rs/russh-sftp) | Subsystem SFTP di atas koneksi `russh` |
| Serial | [`tokio-serial`](https://docs.rs/tokio-serial) | Wrapper async lintas-platform di atas `serialport` |
| Emulasi terminal | [`alacritty_terminal`](https://github.com/alacritty/alacritty) | Parser VTE/xterm asli |
| Penyimpanan | [`rusqlite`](https://docs.rs/rusqlite) (bundled) | SQLite lokal, tanpa dependency sistem tambahan |
| Kripto | [`argon2`](https://docs.rs/argon2), [`chacha20poly1305`](https://docs.rs/chacha20poly1305) | Derive key master password + enkripsi AEAD |
| Import config | [`roxmltree`](https://docs.rs/roxmltree) | Parsing `config.xml` SecureCRT |

## Struktur Proyek

```text
terminus/
├── Cargo.toml                # workspace root — semua versi dependency dipusatkan di sini
├── crates/
│   ├── core/                  # domain model murni (HostProfile, HostGroup, dst) — tanpa I/O
│   ├── vault/                 # penyimpanan terenkripsi (Argon2id + ChaCha20-Poly1305)
│   ├── ssh-engine/            # transport SSH, wrapper russh
│   ├── sftp-engine/           # transfer file SFTP, wrapper russh-sftp (reuse koneksi ssh-engine)
│   ├── serial-engine/         # transport serial/console lintas-platform, wrapper tokio-serial
│   ├── term-emulator/         # parsing VTE (escape sequence) via alacritty_terminal
│   └── app/                   # binary utama: UI Slint + wiring semua crate di atas
├── ui/                        # file .slint (UI declarative, dicompile lewat build.rs di crates/app)
│   ├── tokens.slint            # design tokens: warna/tipografi/spacing/radius (1 sumber kebenaran)
│   ├── components/             # NavRail, TopBar, HostCard, FileRow, TerminalView, ConsoleView, dst
│   └── pages/                  # PageHosts, PageSftp, PageConsole, PageTerminal
├── packaging/                  # script build .deb/.rpm/AppImage (Linux) & .app/.dmg (macOS)
└── docs/                       # situs dokumentasi ini
```

Alur dependency antar crate **searah** — `core` tidak pernah depend ke
crate lain, semua crate lain depend ke `core`.

## Prinsip Desain

- **Tiap crate = satu tanggung jawab.** `core` murni domain model tanpa
  I/O — ini menjaganya tetap ringan dan gampang ditest.
- **Tidak ada plaintext secret di luar `vault`.** `HostProfile` hanya
  menyimpan `credential_id` (UUID); nilai rahasianya hidup di
  `terminus-vault` dan hanya didecrypt saat runtime, tepat sebelum
  connect.
- **UI thread vs async runtime terpisah.** Slint jalan di main thread
  (event loop native OS); semua kerja jaringan/vault jalan di runtime
  Tokio terpisah, dikomunikasikan lewat `slint::invoke_from_event_loop`
  (lihat `crates/app/src/state.rs`).
- **UI = satu sumber kebenaran design token.** Semua warna/spacing/
  radius di `ui/**/*.slint` diambil dari `global Tokens` di
  `ui/tokens.slint` — tidak ada hex/px literal tersebar di komponen.

## Isu Dependency yang Sudah Diperbaiki

`terminus-ssh-engine` depend ke `russh 0.58`, yang secara transitif
menarik `rsa 0.10.0-rc.12` — sebuah **release-candidate** yang
mensyaratkan versi pre-release spesifik tiga crate RustCrypto lain
(`pkcs8 0.11.0-rc.8`, `spki 0.8.0-rc.4`, `der 0.8.0-rc.9`). Kalau
resolver Cargo dibiarkan pilih bebas (`cargo update` tanpa `-p`, atau
`Cargo.lock` dihapus), dia akan menarik versi **final** yang tidak
kompatibel dan build akan gagal.

**Fix-nya sudah dipin di `Cargo.lock`** (jangan dihapus/di-gitignore).
Kalau suatu saat kejadian lagi (habis `cargo update` penuh, atau
upgrade `russh`):

```bash
cargo update -p pkcs8@0.11.0 --precise 0.11.0-rc.8
cargo update -p spki@0.8.0 --precise 0.8.0-rc.4
cargo update -p der@0.8.1 --precise 0.8.0-rc.9
cargo build --workspace   # pastikan hijau lagi
```

(Urutan penting: `spki` harus dipin dulu sebelum `der`.)
