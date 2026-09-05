# Terminus Backend API

Backend self-hosted OPSIONAL buat sinkronisasi vault Terminus (host/
grup/identity/password) antar device. Baca **[DESIGN.md](./DESIGN.md)**
dulu buat arsitektur lengkap (kenapa project ini terpisah dari desktop
app, tech stack, skema database, alur API, urutan pengerjaan) sebelum
menyentuh kode di sini.

## Jalanin lokal (development)

1. `cp .env.example .env` lalu isi nilai sungguhan (`DATABASE_URL`,
   `SERVER_MASTER_KEY`, `JWT_SECRET` — lihat komentar di dalam
   `.env.example` cara generate key-nya).
2. `npm install`
3. `npm run prisma:migrate` — bikin/update tabel di database sesuai
   `prisma/schema.prisma`.
4. `npm run dev` — jalan di `http://localhost:4000` (atau `PORT` lain
   yang diisi di `.env`), auto-restart tiap ada perubahan file.
5. Cek `GET /health` — harus balikin `{"status":"ok","db":"connected"}`
   kalau koneksi database benar.

## Test

```
npm test
```

## Self-host lewat Docker

`docker-compose.yml` di sini CUMA berisi service `server` — MySQL-nya
TIDAK dibundle (harus sudah ada sendiri, mis. instance produksi yang
sudah jalan terpisah). Baca komentar di `docker-compose.yml` buat 2
cara umum nyambungin ke MySQL yang sudah ada itu (lewat host port yang
dia publish, atau gabung ke network Docker eksternal tempat dia hidup).

```
cp .env.example .env   # isi DATABASE_URL ke MySQL yang sudah ada, dst
docker compose up -d --build
```
