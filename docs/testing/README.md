# Testing

```bash
# Semua unit test lintas-crate (cepat, tidak butuh jaringan/hardware)
cargo test --workspace
```

Dua test integrasi CONNECT SUNGGUHAN (bukan mock) ke server SSH lokal
dikecualikan secara default (`#[ignore]`) karena butuh Docker:

```bash
docker run -d --name terminus-test-sshd -p 2222:22 alpine:3.20 sh -c "
  apk add --no-cache openssh >/dev/null 2>&1
  echo 'root:testpass123' | chpasswd
  sed -i 's/#PermitRootLogin.*/PermitRootLogin yes/' /etc/ssh/sshd_config
  sed -i 's/#PasswordAuthentication.*/PasswordAuthentication yes/' /etc/ssh/sshd_config
  ssh-keygen -A >/dev/null 2>&1
  /usr/sbin/sshd -D
"
cargo test -p terminus-ssh-engine --test end_to_end -- --ignored
cargo test -p terminus-app --test ssh_terminal_e2e -- --ignored

# beres testing:
docker rm -f terminus-test-sshd
```

## Apa yang Dites

- `terminus-vault` — enkripsi/dekripsi roundtrip, penolakan password
  salah, isolasi read-tanpa-unlock, cascade delete grup.
- `terminus-term-emulator` — parsing escape sequence ANSI jadi grid
  warna, resize, snapshot lintas tema.
- `terminus-core` — parser import SecureCRT (flatten grup bertingkat,
  dedup, filter protokol non-SSH2).
- `terminus-app` — simulasi alur UI Hosts penuh (create/delete host &
  grup, invarian List/Groups) lewat wiring Rust asli + konversi
  grid→Slint, dijalankan di dalam event loop Slint sungguhan
  (`slint::run_event_loop_until_quit()`).
- **End-to-end** (`--ignored`, butuh Docker) — koneksi SSH sungguhan:
  password ke-decrypt benar dari vault, channel shell dua-arah jalan,
  host-key mismatch ditolak, sinyal disconnect terkirim, DAN command
  yang diketik lewat SSH sungguhan benar-benar ter-render di grid
  terminal (bukan cuma diterima sebagai byte mentah).
