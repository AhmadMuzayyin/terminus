# Terminus Backend API — Desain Arsitektur

> Dokumen ini adalah SUMBER KEBENARAN buat struktur folder & alur kerja
> project `backend/` ini. WAJIB dibaca ulang setiap mau melanjutkan
> kerjaan di folder ini (termasuk lintas sesi kerja) SEBELUM menulis
> kode baru — supaya konvensi (nama folder, pola per-modul, urutan
> pengerjaan) tetap konsisten dan tidak improvisasi berbeda-beda tiap
> kali disentuh. Update dokumen ini juga kalau ada keputusan desain
> yang berubah selama pengerjaan (jangan biarkan dokumen ini basi).

## 1. Apa ini & kenapa terpisah dari desktop app

`backend/` adalah **project Node.js yang SEPENUHNYA TERPISAH** dari
aplikasi desktop Terminus (`crates/`, `ui/`) — beda proses, beda
deployment, beda bahasa. Kebetulan hidup di git repo yang sama supaya
gampang dikelola bareng, TAPI:

- Cargo (workspace Rust di `Cargo.toml` root) TIDAK menyentuh folder
  ini sama sekali — `members` di workspace itu daftar eksplisit
  (bukan glob `crates/*`), jadi aman.
- Folder ini punya `package.json`/`node_modules` sendiri, tooling
  sendiri, siklus build/deploy sendiri (Docker image sendiri).

**Ada 3 aplikasi berbeda secara total dalam ekosistem Terminus:**

1. **Desktop app** (`crates/`+`ui/`, SUDAH ADA) — install seperti
   biasa, jalan penuh TANPA backend ini (mode "Local").
2. **Android app** (BELUM ADA, roadmap terpisah setelah backend ini).
3. **Backend API** (folder ini) — server OPSIONAL yang di-self-host
   sendiri oleh user (VPS/NAS/rumah), dipakai desktop/Android HANYA
   kalau user pilih mode "Self-hosted" (masukkan URL server + login)
   alih-alih mode "Local" (`vault.db` lokal, seperti sekarang).

Kalau user pilih Self-hosted di desktop DAN di Android, keduanya
connect ke server yang SAMA → otomatis "sinkron" karena satu sumber
data yang sama, bukan lewat mekanisme sync-antar-file.

## 2. Tech Stack & Alasan

| Bagian | Pilihan | Alasan |
|---|---|---|
| Bahasa | **TypeScript** (Node.js) | Backend ini nyimpan & memproses password host — type safety mengurangi kelas bug ceroboh (salah kirim field, dsb) dibanding JS polos. |
| Web framework | **Express** | Simpel, ekosistem besar, tidak banyak "magic" (decorator/DI berat ala NestJS) — struktur tetap dijaga lewat KONVENSI folder per-modul di bawah (bagian 3), bukan lewat framework yang maksa. Selaras juga dengan gaya desktop app yang lebih suka eksplisit daripada abstraksi berat. |
| Database | **MySQL** | Sesuai keputusan eksplisit user. |
| Akses DB | **Prisma** (`schema.prisma` + generated client) | Type-safe query result, DAN sudah termasuk sistem migrasi (`prisma migrate`) — pas buat kebutuhan "struktur yang jelas & bisa diulang" tanpa nulis migration runner sendiri. |
| Hash password login | **argon2** (npm `argon2`) | SAMA ALGORITMA dengan `terminus-vault::crypto::derive_key` di desktop app — konsisten postur keamanan lintas project. |
| Enkripsi password host (`secrets.data`) | **ChaCha20-Poly1305** lewat modul bawaan `node:crypto` | SAMA PRIMITIVE dengan `terminus-vault::crypto::encrypt/decrypt`. Key-nya BEDA SUMBER dari vault lokal: di sini SATU kunci milik SERVER (`SERVER_MASTER_KEY`, env var, di-generate sekali waktu setup) — konsekuensi keputusan "server-side encryption at rest" yang sudah disepakati (bukan zero-knowledge per-user, server dipercaya karena self-hosted). |
| Access token | **JWT** (npm `jsonwebtoken`), umur pendek (~15 menit) | Standar, stateless, gampang diverifikasi di middleware. |
| Refresh token | Random string, DI-HASH lalu disimpan di tabel `refresh_tokens` | JWT stateless TIDAK BISA di-revoke — refresh token yang stateful di DB bisa, penting buat fitur "Logout" beneran & pencabutan akses. |
| Validasi request | **zod** | Skema TypeScript-first, dipakai buat validasi body/query di setiap endpoint sebelum masuk ke logic bisnis. |
| Test | **vitest** | Modern, cepat, cocok TypeScript/ESM. |
| Lint/format | ESLint + Prettier | Standar. |
| Deployment | Docker image + `docker-compose.yml` (bundling MySQL) | Self-host tinggal `docker compose up`. |

## 3. Struktur Folder

```
backend/
  DESIGN.md                  # dokumen ini
  README.md                  # cara jalanin dev/prod, daftar env var
  package.json
  tsconfig.json               # dipakai editor/vitest (mencakup src/ + tests/)
  tsconfig.build.json          # dipakai `npm run build` — extends tsconfig.json, rootDir+include DIPERSEMPIT ke src/ saja
  .env.example
  Dockerfile
  docker-compose.yml
  prisma/
    schema.prisma            # skema tabel (lihat bagian 4)
    migrations/              # riwayat migrasi (auto oleh `prisma migrate`)
  src/
    index.ts                 # entrypoint: load env, connect DB, start server
    app.ts                   # buat Express app + pasang middleware global
    errors.ts                 # hierarki HttpError (BadRequestError/UnauthorizedError/dst) — dilempar service, ditangkap errorHandler
    config/
      env.ts                 # baca+validasi env var (pakai zod), export satu objek config
    db/
      client.ts              # singleton PrismaClient
    crypto/
      password.ts            # hash/verify password login (argon2)
      secrets.ts             # encrypt/decrypt password host (ChaCha20-Poly1305 + SERVER_MASTER_KEY)
      tokens.ts               # generate/verify access token (JWT) + refresh token (random+hash)
    middleware/
      auth.ts                 # requireAuth: verifikasi JWT dari header Authorization, isi req.user
      vaultAccess.ts           # requireVaultMember: cek req.user anggota :vaultId di URL
      validate.ts              # bungkus skema zod jadi middleware validasi
      errorHandler.ts          # error handler terpusat -> response JSON konsisten
    utils/
      asyncHandler.ts          # bungkus handler async controller supaya rejection-nya nyampe ke errorHandler (Express 4 tidak nangkap otomatis)
    modules/                   # SATU FOLDER PER DOMAIN, pola 4-file KONSISTEN semua modul:
      auth/
        auth.routes.ts         #   - routes.ts    : daftarin endpoint ke express.Router() (termasuk GET /me, protected requireAuth — cara termudah verifikasi access token valid)
        auth.controller.ts     #   - controller.ts: terima request, panggil service, bentuk response
        auth.service.ts        #   - service.ts   : logic bisnis murni, TIDAK tahu soal HTTP (gampang unit test)
        auth.schema.ts         #   - schema.ts    : skema zod buat validasi body/query
      vaults/
        vaults.routes.ts
        vaults.controller.ts
        vaults.service.ts
        vaults.schema.ts
      hosts/
        hosts.routes.ts
        hosts.controller.ts
        hosts.service.ts
        hosts.schema.ts
      groups/
        groups.routes.ts
        groups.controller.ts
        groups.service.ts
        groups.schema.ts
      identities/
        identities.routes.ts
        identities.controller.ts
        identities.service.ts
        identities.schema.ts
    types/
      express.d.ts             # augmentasi Express.Request nambah field `user`/`vaultRole`
  tests/
    auth.test.ts
    vaults.test.ts
    hosts.test.ts
    groups.test.ts
    identities.test.ts
```

**Aturan konvensi yang WAJIB dipatuhi tiap nambah modul baru** (biar
predictable buat dibaca ulang lintas sesi):
- Modul baru = folder baru di `src/modules/<nama>/` dengan 4 file
  (`*.routes.ts`, `*.controller.ts`, `*.service.ts`, `*.schema.ts`) —
  jangan campur logic bisnis ke dalam file routes/controller.
- `*.service.ts` TIDAK BOLEH import apa pun dari Express (`Request`/
  `Response`) — biar gampang dites tanpa perlu mock HTTP.
- Semua akses tabel lewat Prisma client (`db/client.ts`), jangan nulis
  raw SQL string kecuali benar-benar terpaksa (beda dari `terminus-
  vault` yang sengaja raw SQL — di Node ekosistemnya lebih umum pakai
  Prisma, biar konsisten sama gaya common Node/TS, bukan maksa niru
  gaya Rust).

## 4. Skema Database (garis besar `schema.prisma`)

| Tabel | Kolom penting | Catatan |
|---|---|---|
| `users` | id (uuid), email (unique), password_hash, created_at | |
| `vaults` | id (uuid), name, owner_user_id | Satu vault = satu unit yang bisa dipakai personal (cuma owner) ATAU tim (banyak member). |
| `vault_members` | vault_id, user_id, role (`owner`\|`member`), created_at | RBAC sederhana — v1 cuma 2 role, belum ada permission granular per-resource. |
| `host_groups` | id, vault_id, name, subtitle, parent_id | Mirror `terminus_core::HostGroup` + `vault_id`. |
| `host_profiles` | id, vault_id, label, host, port, username, kind, auth_method, credential_id, group_id, tags (JSON), terminal_theme | Mirror `terminus_core::HostProfile` + `vault_id`. Field-field ini PLAINTEXT (sama seperti SQLite lokal sekarang) — cuma password yang lewat `secrets`. |
| `identities` | id, vault_id, label, username, credential_id | Mirror `terminus_core::Identity` + `vault_id`. |
| `secrets` | credential_id (PK), vault_id, encrypted_data (bytes) | `encrypted_data` = hasil `crypto/secrets.ts` (ChaCha20-Poly1305 + `SERVER_MASTER_KEY`). TIDAK PERNAH plaintext di kolom ini. |
| `refresh_tokens` | id, user_id, token_hash, expires_at, revoked_at | Dipakai `POST /auth/refresh` & `POST /auth/logout`. |

Field-field `host_profiles`/`host_groups`/`identities` SENGAJA dibuat
SEJAJAR (nama & makna sama) dengan struct Rust `terminus-core` yang
sudah ada (`HostProfile`, `HostGroup`, `Identity`) — supaya nanti waktu
desktop app diintegrasikan (fase terpisah, lihat bagian 7), pemetaan
JSON API <-> struct Rust itu 1:1, tidak perlu transformasi rumit.

## 5. Alur API

### 5.1 Setup awal (sekali, waktu self-host pertama kali)
Admin jalankan `docker compose up` dengan env `SERVER_MASTER_KEY` (buat
enkripsi `secrets.encrypted_data`) dan `JWT_SECRET` (buat tandatangan
access token) sudah diisi. Database masih kosong.

### 5.2 Register & Login
- `POST /api/v1/auth/register` — **CUMA aktif kalau tabel `users` masih
  kosong** (first-run bootstrap admin). Setelah user pertama ada,
  endpoint ini nolak request baru (403) — registrasi selanjutnya HARUS
  lewat invite (lihat 5.3), bukan open registration, karena ini alat
  self-hosted bukan layanan publik.
- `POST /api/v1/auth/login` (email+password) → verifikasi argon2 →
  balikin `{ accessToken, refreshToken }`.
- `POST /api/v1/auth/refresh` (refreshToken) → cek valid & belum
  revoked di tabel `refresh_tokens` → balikin accessToken baru
  (+ refreshToken baru, rotate).
- `POST /api/v1/auth/logout` (refreshToken) → tandai `revoked_at` di DB.
- `GET /api/v1/auth/me` (protected, `requireAuth`) — endpoint tambahan
  kecil di luar daftar awal: balikin `{id, email}` user yang login,
  cara termudah verifikasi access token valid dari sisi client MAUPUN
  test.

### 5.3 Vault & Membership
- `POST /api/v1/vaults` (butuh login) → bikin vault baru, pembuat
  otomatis jadi `owner` di `vault_members`.
- `GET /api/v1/vaults` → daftar vault yang user ini jadi anggotanya.
- `POST /api/v1/vaults/:vaultId/members` (owner-only) → tambah member
  (by email user yang SUDAH terdaftar — alur invite-buat-user-baru-
  yang-belum-terdaftar didetailkan nanti waktu dikerjakan, bukan
  sekarang).
- `DELETE /api/v1/vaults/:vaultId/members/:userId` (owner-only) →
  cabut akses.

### 5.4 CRUD host/grup/identity
Semua endpoint di bawah **WAJIB lewat middleware `requireAuth` LALU
`requireVaultMember`** (cek `:vaultId` di URL, user harus anggota):

- `GET/POST/PUT/DELETE /api/v1/vaults/:vaultId/hosts[/:id]`
- `GET/POST/PUT/DELETE /api/v1/vaults/:vaultId/groups[/:id]`
- `GET/POST/PUT/DELETE /api/v1/vaults/:vaultId/identities[/:id]`

Metadata (label/host/port/username/tags/nama grup) disimpan &
dikembalikan PLAINTEXT lewat endpoint-endpoint ini — cukup diamankan
lewat HTTPS in-transit + auth, sama seperti metadata plaintext di
SQLite lokal sekarang.

### 5.5 Password host (secret)
- `PUT /api/v1/vaults/:vaultId/hosts/:id/secret` (body: `{ password }`)
  → server enkripsi (`crypto/secrets.ts`) → simpan ke `secrets.
  encrypted_data`.
- `GET /api/v1/vaults/:vaultId/hosts/:id/secret` → server dekripsi →
  balikin plaintext SEKALI PAKAI ke client lewat HTTPS (dipanggil app
  cuma waktu benar-benar mau connect SSH, mirror `vault.read_secret`
  di desktop app sekarang) — TIDAK PERNAH ikut di response list/GET
  host biasa (supaya password tidak ke-fetch tanpa perlu tiap kali
  render daftar host).

## 6. Urutan Pengerjaan (Milestone)

Dikerjakan SATU-SATU (konsisten dengan cara kerja proyek desktop-nya),
tiap milestone selesai + terverifikasi (test hijau) sebelum lanjut:

1. ✅ **Scaffold project** — `package.json`, `tsconfig.json`, ESLint/
   Prettier, `prisma/schema.prisma` (semua tabel bagian 4), Docker +
   docker-compose, `.env.example`, `src/config/env.ts`, `src/app.ts`
   kosong (cuma health-check `GET /health`). Diverifikasi lawan MySQL
   sungguhan (container `mesem-mysql`) — migrasi jalan, build/lint/test
   hijau, dev server dites manual lewat curl.
2. ✅ **Auth module** — register(first-run)/login/refresh/logout +
   `GET /me` penuh, diverifikasi lawan MySQL sungguhan (test + curl
   manual). Nambah `src/errors.ts`, `src/utils/asyncHandler.ts`,
   `tsconfig.build.json` (lihat bagian 3) yang tidak ada di rencana
   awal — kebutuhan nyata yang muncul waktu dikerjakan.
3. **Vaults module** — create vault, list vault, add/remove member +
   test.
4. **Hosts/Groups/Identities module** — CRUD penuh + endpoint secret +
   test.
5. **Docker packaging final** — pastikan `docker compose up` dari nol
   beneran jalan end-to-end (dites manual).

## 7. Sengaja DI LUAR SCOPE sekarang (jangan dikerjakan tanpa diminta)

- Integrasi ke desktop app (`crates/app`) supaya bisa pilih Local vs
  Self-hosted — ini PEKERJAAN TERPISAH di sisi Rust (butuh bikin
  abstraksi storage backend baru di sana), baru dikerjakan SETELAH
  backend ini sendiri selesai & stabil.
- Android app.
- Permission granular per-host di dalam satu vault tim (v1: semua
  member satu vault lihat semua isi vault itu).
- Invite user yang BELUM punya akun (v1: add member cuma buat email
  yang sudah terdaftar).
