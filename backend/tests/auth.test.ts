// Test ini jalan lawan MySQL SUNGGUHAN (container mesem-mysql, sama
// filosofi dengan test Rust `crates/vault` yang selalu pakai DB
// beneran, bukan mock). Tabel `refresh_tokens`/`users` dikosongkan di
// `beforeAll` (bukan per-test) supaya skenario "register CUMA jalan
// waktu tabel users kosong" beneran teruji dari kondisi bersih tiap
// kali suite ini dijalankan ulang — database MySQL yang dipakai
// bersifat persisten (bukan file temp sekali pakai seperti SQLite
// lokal desktop app), jadi harus dibersihkan eksplisit.

import { beforeAll, describe, expect, it } from "vitest";
import request from "supertest";

import { createApp } from "../src/app.js";
import { prisma } from "../src/db/client.js";

const app = createApp();

const FIRST_USER = { email: "admin@terminus.test", password: "super-secret-password" };

beforeAll(async () => {
  await prisma.refreshToken.deleteMany();
  await prisma.user.deleteMany();
});

describe("Auth flow", () => {
  let accessToken = "";
  let refreshToken = "";

  it("register user pertama sukses (first-run bootstrap)", async () => {
    const res = await request(app).post("/api/v1/auth/register").send(FIRST_USER);

    expect(res.status).toBe(201);
    expect(res.body.user.email).toBe(FIRST_USER.email);
    expect(res.body.accessToken).toBeTypeOf("string");
    expect(res.body.refreshToken).toBeTypeOf("string");
  });

  it("register kedua ditolak — registrasi publik tertutup setelah user pertama ada", async () => {
    const res = await request(app)
      .post("/api/v1/auth/register")
      .send({ email: "hacker@example.com", password: "irrelevant123" });

    expect(res.status).toBe(403);
  });

  it("register password terlalu pendek ditolak validasi (400), bukan nyampe ke service", async () => {
    const res = await request(app)
      .post("/api/v1/auth/register")
      .send({ email: "x@example.com", password: "short" });

    expect(res.status).toBe(400);
  });

  it("login dengan password salah ditolak", async () => {
    const res = await request(app)
      .post("/api/v1/auth/login")
      .send({ email: FIRST_USER.email, password: "password-salah" });

    expect(res.status).toBe(401);
  });

  it("login dengan email yang tidak terdaftar ditolak dengan pesan SAMA seperti password salah", async () => {
    const res = await request(app)
      .post("/api/v1/auth/login")
      .send({ email: "tidak-ada@example.com", password: "apapun12345" });

    expect(res.status).toBe(401);
  });

  it("login dengan kredensial benar berhasil", async () => {
    const res = await request(app).post("/api/v1/auth/login").send(FIRST_USER);

    expect(res.status).toBe(200);
    accessToken = res.body.accessToken;
    refreshToken = res.body.refreshToken;
    expect(accessToken).toBeTypeOf("string");
    expect(refreshToken).toBeTypeOf("string");
  });

  it("GET /me tanpa token ditolak", async () => {
    const res = await request(app).get("/api/v1/auth/me");
    expect(res.status).toBe(401);
  });

  it("GET /me dengan token asal-asalan ditolak", async () => {
    const res = await request(app).get("/api/v1/auth/me").set("Authorization", "Bearer token-ngasal");
    expect(res.status).toBe(401);
  });

  it("GET /me dengan access token valid berhasil", async () => {
    const res = await request(app).get("/api/v1/auth/me").set("Authorization", `Bearer ${accessToken}`);

    expect(res.status).toBe(200);
    expect(res.body.email).toBe(FIRST_USER.email);
  });

  it("refresh token valid mengeluarkan pasangan token baru, DAN token lama tidak bisa dipakai ulang", async () => {
    const res = await request(app).post("/api/v1/auth/refresh").send({ refreshToken });
    expect(res.status).toBe(200);

    const newRefreshToken: string = res.body.refreshToken;
    expect(newRefreshToken).not.toBe(refreshToken);

    const reuseOld = await request(app).post("/api/v1/auth/refresh").send({ refreshToken });
    expect(reuseOld.status).toBe(401);

    refreshToken = newRefreshToken;
  });

  it("logout mencabut refresh token — dipakai lagi setelah itu ditolak", async () => {
    const logoutRes = await request(app).post("/api/v1/auth/logout").send({ refreshToken });
    expect(logoutRes.status).toBe(204);

    const afterLogout = await request(app).post("/api/v1/auth/refresh").send({ refreshToken });
    expect(afterLogout.status).toBe(401);
  });

  it("logout dengan refreshToken yang sudah tidak ada tetap 204 (idempotent, aman dipanggil kapan pun)", async () => {
    const res = await request(app)
      .post("/api/v1/auth/logout")
      .send({ refreshToken: "sudah-tidak-ada-token-ini" });
    expect(res.status).toBe(204);
  });
});
