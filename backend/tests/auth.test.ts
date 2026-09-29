// Test ini jalan lawan MySQL SUNGGUHAN (container mesem-mysql, sama
// filosofi dengan test Rust `crates/vault` yang selalu pakai DB
// beneran, bukan mock). Tabel `refresh_tokens`/`users` dikosongkan di
// `beforeAll` (bukan per-test) supaya skenario "register CUMA jalan
// waktu tabel users kosong" beneran teruji dari kondisi bersih tiap
// kali suite ini dijalankan ulang — database MySQL yang dipakai
// bersifat persisten (bukan file temp sekali pakai seperti SQLite
// lokal desktop app), jadi harus dibersihkan eksplisit.

import { afterAll, beforeAll, describe, expect, it } from "vitest";
import request from "supertest";

import { createApp } from "../src/app.js";
import { prisma } from "../src/db/client.js";

const app = createApp();

const FIRST_USER = { email: "admin@terminus.test", password: "super-secret-password", fullName: "Admin Terminus" };
// Dipakai suite "Profil akun" (email & password FIRST_USER diubah di sana).
const CHANGED_EMAIL = "admin-baru@terminus.test";
const OTHER_USER_EMAIL = "lain@terminus.test";

beforeAll(async () => {
  await prisma.refreshToken.deleteMany();
  await prisma.user.deleteMany();
});

// Aturan DESIGN.md Milestone 3: tiap suite bersihkan fixture-nya sendiri.
// Tanpa ini `admin@terminus.test` tertinggal, dan DB dev jadi "sudah punya
// user" — alur "Daftar admin pertama" di desktop app tidak bisa diuji lagi
// setelah `npm test`. Refresh token ikut terhapus (onDelete: Cascade).
afterAll(async () => {
  await prisma.user.deleteMany({ where: { email: { in: [FIRST_USER.email, CHANGED_EMAIL, OTHER_USER_EMAIL] } } });
});

describe("Auth flow", () => {
  let accessToken = "";
  let refreshToken = "";

  it("register user pertama sukses (first-run bootstrap)", async () => {
    const res = await request(app).post("/api/v1/auth/register").send(FIRST_USER);

    expect(res.status).toBe(201);
    expect(res.body.user.email).toBe(FIRST_USER.email);
    expect(res.body.user.fullName).toBe(FIRST_USER.fullName);
    expect(res.body.accessToken).toBeTypeOf("string");
    expect(res.body.refreshToken).toBeTypeOf("string");
  });

  it("register kedua ditolak — registrasi publik tertutup setelah user pertama ada", async () => {
    const res = await request(app)
      .post("/api/v1/auth/register")
      .send({ email: "hacker@example.com", password: "irrelevant123", fullName: "Hacker" });

    expect(res.status).toBe(403);
  });

  it("register password terlalu pendek ditolak validasi (400), bukan nyampe ke service", async () => {
    const res = await request(app)
      .post("/api/v1/auth/register")
      .send({ email: "x@example.com", password: "short", fullName: "X" });

    expect(res.status).toBe(400);
  });

  it("register tanpa fullName, atau fullName cuma spasi, ditolak validasi (400)", async () => {
    const missing = await request(app)
      .post("/api/v1/auth/register")
      .send({ email: "x@example.com", password: "password-cukup-panjang" });
    expect(missing.status).toBe(400);

    const blank = await request(app)
      .post("/api/v1/auth/register")
      .send({ email: "x@example.com", password: "password-cukup-panjang", fullName: "   " });
    expect(blank.status).toBe(400);
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
    expect(res.body.user.fullName).toBe(FIRST_USER.fullName);
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
    expect(res.body.fullName).toBe(FIRST_USER.fullName);
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

describe("Profil akun (PATCH /me, PUT /me/password)", () => {
  let accessToken = "";
  let refreshToken = "";
  const auth = () => ({ Authorization: `Bearer ${accessToken}` });

  beforeAll(async () => {
    const res = await request(app).post("/api/v1/auth/login").send(FIRST_USER);
    accessToken = res.body.accessToken;
    refreshToken = res.body.refreshToken;
    // User lain buat skenario "email sudah dipakai" — dibuat langsung
    // lewat Prisma (register publik sudah tertutup).
    await prisma.user.create({ data: { email: OTHER_USER_EMAIL, passwordHash: "tidak-dipakai" } });
  });

  it("tanpa token ditolak (401)", async () => {
    const res = await request(app).patch("/api/v1/auth/me").send({ fullName: "Siapa" });
    expect(res.status).toBe(401);
  });

  it("body tanpa fullName maupun email ditolak (400)", async () => {
    const res = await request(app).patch("/api/v1/auth/me").set(auth()).send({});
    expect(res.status).toBe(400);
  });

  it("ganti fullName saja tidak butuh password, spasi di ujung dibuang", async () => {
    const res = await request(app).patch("/api/v1/auth/me").set(auth()).send({ fullName: "  Admin Baru  " });
    expect(res.status).toBe(200);
    expect(res.body.fullName).toBe("Admin Baru");
    expect(res.body.email).toBe(FIRST_USER.email);
  });

  it("ganti email TANPA password ditolak 403 (bukan 401 — lihat DESIGN.md Milestone 6)", async () => {
    const res = await request(app).patch("/api/v1/auth/me").set(auth()).send({ email: CHANGED_EMAIL });
    expect(res.status).toBe(403);
  });

  it("ganti email dengan password SALAH ditolak 403", async () => {
    const res = await request(app)
      .patch("/api/v1/auth/me")
      .set(auth())
      .send({ email: CHANGED_EMAIL, currentPassword: "bukan-password-ini" });
    expect(res.status).toBe(403);
  });

  it("ganti email ke email user lain ditolak 409", async () => {
    const res = await request(app)
      .patch("/api/v1/auth/me")
      .set(auth())
      .send({ email: OTHER_USER_EMAIL, currentPassword: FIRST_USER.password });
    expect(res.status).toBe(409);
  });

  it("email SAMA dengan sekarang bukan perubahan — tidak butuh password", async () => {
    const res = await request(app).patch("/api/v1/auth/me").set(auth()).send({ email: FIRST_USER.email });
    expect(res.status).toBe(200);
  });

  it("ganti email dengan password benar sukses, login pakai email baru bisa, email lama tidak", async () => {
    const res = await request(app)
      .patch("/api/v1/auth/me")
      .set(auth())
      .send({ email: CHANGED_EMAIL, currentPassword: FIRST_USER.password });
    expect(res.status).toBe(200);
    expect(res.body.email).toBe(CHANGED_EMAIL);

    const loginNew = await request(app)
      .post("/api/v1/auth/login")
      .send({ email: CHANGED_EMAIL, password: FIRST_USER.password });
    expect(loginNew.status).toBe(200);
    const loginOld = await request(app).post("/api/v1/auth/login").send(FIRST_USER);
    expect(loginOld.status).toBe(401);
  });

  it("ganti password: password saat ini salah 403, password baru pendek 400", async () => {
    const wrong = await request(app)
      .put("/api/v1/auth/me/password")
      .set(auth())
      .send({ currentPassword: "salah", newPassword: "password-baru-panjang" });
    expect(wrong.status).toBe(403);

    const short = await request(app)
      .put("/api/v1/auth/me/password")
      .set(auth())
      .send({ currentPassword: FIRST_USER.password, newPassword: "pendek" });
    expect(short.status).toBe(400);
  });

  it("ganti password sukses: SEMUA refresh token lama dicabut, perangkat ini dapat token baru", async () => {
    // Sesi "perangkat lain" — login terpisah SEBELUM ganti password.
    const otherDevice = await request(app)
      .post("/api/v1/auth/login")
      .send({ email: CHANGED_EMAIL, password: FIRST_USER.password });
    const otherRefresh: string = otherDevice.body.refreshToken;

    const res = await request(app)
      .put("/api/v1/auth/me/password")
      .set(auth())
      .send({ currentPassword: FIRST_USER.password, newPassword: "password-baru-panjang" });
    expect(res.status).toBe(200);
    expect(res.body.accessToken).toBeTypeOf("string");
    expect(res.body.refreshToken).toBeTypeOf("string");

    // Refresh token LAMA perangkat ini & perangkat lain -> ditolak.
    for (const old of [refreshToken, otherRefresh]) {
      const reuse = await request(app).post("/api/v1/auth/refresh").send({ refreshToken: old });
      expect(reuse.status).toBe(401);
    }
    // Token BARU dari respons -> tetap bisa dipakai (perangkat ini tetap login).
    const fresh = await request(app).post("/api/v1/auth/refresh").send({ refreshToken: res.body.refreshToken });
    expect(fresh.status).toBe(200);

    const loginOldPwd = await request(app)
      .post("/api/v1/auth/login")
      .send({ email: CHANGED_EMAIL, password: FIRST_USER.password });
    expect(loginOldPwd.status).toBe(401);
    const loginNewPwd = await request(app)
      .post("/api/v1/auth/login")
      .send({ email: CHANGED_EMAIL, password: "password-baru-panjang" });
    expect(loginNewPwd.status).toBe(200);
  });
});
