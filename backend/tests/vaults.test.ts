// Beda dari auth.test.ts: suite ini SENGAJA bikin user langsung lewat
// Prisma (bukan lewat POST /auth/register, yang cuma bisa dipakai
// SEKALI seumur hidup server) supaya SELF-CONTAINED — tidak bergantung
// urutan file test lain dijalankan atau state yang ditinggalkannya.
// Dibersihkan dulu di awal biar idempotent kalau suite ini dijalankan
// ulang. Lihat vitest.config.ts (`fileParallelism: false`) — WAJIB
// supaya file test lain (auth.test.ts, yang mengosongkan tabel users)
// tidak jalan bersamaan lawan database sungguhan yang sama ini.

import { afterAll, beforeAll, describe, expect, it } from "vitest";
import request from "supertest";

import { createApp } from "../src/app.js";
import { hashPassword } from "../src/crypto/password.js";
import { signAccessToken } from "../src/crypto/tokens.js";
import { prisma } from "../src/db/client.js";

const app = createApp();

const OWNER = { email: "owner-vaults-test@terminus.test" };
const MEMBER_CANDIDATE = { email: "member-vaults-test@terminus.test" };

let ownerToken = "";
let ownerId = "";
let memberCandidateId = "";

beforeAll(async () => {
  const testEmails = [OWNER.email, MEMBER_CANDIDATE.email];
  await prisma.vaultMember.deleteMany({ where: { user: { email: { in: testEmails } } } });
  await prisma.vault.deleteMany({ where: { owner: { email: OWNER.email } } });
  await prisma.user.deleteMany({ where: { email: { in: testEmails } } });

  const owner = await prisma.user.create({
    data: { email: OWNER.email, passwordHash: await hashPassword("owner-password-123") },
  });
  const memberCandidate = await prisma.user.create({
    data: { email: MEMBER_CANDIDATE.email, passwordHash: await hashPassword("member-password-123") },
  });

  ownerId = owner.id;
  memberCandidateId = memberCandidate.id;
  ownerToken = signAccessToken(owner.id);
});

// WAJIB — auth.test.ts (file test lain) menghapus SEMUA baris tabel
// `users` di beforeAll-nya (buat menguji "register cuma jalan waktu
// tabel users kosong"). Kalau user yang dibikin suite ini dibiarkan
// nyangkut, `auth.test.ts` bakal gagal dengan foreign key constraint
// error (`Vault.owner_user_id` TIDAK ber-cascade, beda dari
// `VaultMember.user` yang cascade) — vault-nya harus dihapus DULUAN,
// baru user-nya aman dihapus.
afterAll(async () => {
  await prisma.vault.deleteMany({ where: { ownerUserId: ownerId } });
  await prisma.user.deleteMany({ where: { id: { in: [ownerId, memberCandidateId] } } });
});

describe("Vaults & membership", () => {
  let vaultId = "";

  it("bikin vault baru -> pembuat otomatis jadi owner", async () => {
    const res = await request(app)
      .post("/api/v1/vaults")
      .set("Authorization", `Bearer ${ownerToken}`)
      .send({ name: "Vault Tim NOC" });

    expect(res.status).toBe(201);
    expect(res.body).toEqual({ id: expect.any(String), name: "Vault Tim NOC", role: "owner" });
    vaultId = res.body.id;
  });

  it("list vault balikin vault yang baru dibuat dengan role owner", async () => {
    const res = await request(app).get("/api/v1/vaults").set("Authorization", `Bearer ${ownerToken}`);

    expect(res.status).toBe(200);
    expect(res.body).toContainEqual({ id: vaultId, name: "Vault Tim NOC", role: "owner" });
  });

  it("tambah member by email user yang sudah terdaftar berhasil", async () => {
    const res = await request(app)
      .post(`/api/v1/vaults/${vaultId}/members`)
      .set("Authorization", `Bearer ${ownerToken}`)
      .send({ email: MEMBER_CANDIDATE.email });

    expect(res.status).toBe(201);
    expect(res.body).toEqual({ id: memberCandidateId, email: MEMBER_CANDIDATE.email, role: "member" });
  });

  it("tambah member yang SAMA dua kali ditolak (sudah jadi anggota)", async () => {
    const res = await request(app)
      .post(`/api/v1/vaults/${vaultId}/members`)
      .set("Authorization", `Bearer ${ownerToken}`)
      .send({ email: MEMBER_CANDIDATE.email });

    expect(res.status).toBe(409);
  });

  it("tambah member dengan email yang belum terdaftar ditolak (404)", async () => {
    const res = await request(app)
      .post(`/api/v1/vaults/${vaultId}/members`)
      .set("Authorization", `Bearer ${ownerToken}`)
      .send({ email: "belum-pernah-daftar-vaults-test@example.com" });

    expect(res.status).toBe(404);
  });

  it("member (BUKAN owner) tidak boleh tambah member lain", async () => {
    const memberToken = signAccessToken(memberCandidateId);
    const res = await request(app)
      .post(`/api/v1/vaults/${vaultId}/members`)
      .set("Authorization", `Bearer ${memberToken}`)
      .send({ email: OWNER.email });

    expect(res.status).toBe(403);
  });

  it("user yang bukan anggota vault sama sekali ditolak (403) waktu akses endpoint member", async () => {
    const outsider = await prisma.user.create({
      data: {
        email: "outsider-vaults-test@terminus.test",
        passwordHash: await hashPassword("irrelevant-pw-123"),
      },
    });
    const outsiderToken = signAccessToken(outsider.id);

    const res = await request(app)
      .post(`/api/v1/vaults/${vaultId}/members`)
      .set("Authorization", `Bearer ${outsiderToken}`)
      .send({ email: OWNER.email });

    expect(res.status).toBe(403);

    await prisma.user.delete({ where: { id: outsider.id } });
  });

  it("vaultId dengan format bukan UUID ditolak validasi (400), tidak sempat query DB", async () => {
    const res = await request(app)
      .post("/api/v1/vaults/bukan-uuid/members")
      .set("Authorization", `Bearer ${ownerToken}`)
      .send({ email: OWNER.email });

    expect(res.status).toBe(400);
  });

  it("hapus member berhasil", async () => {
    const res = await request(app)
      .delete(`/api/v1/vaults/${vaultId}/members/${memberCandidateId}`)
      .set("Authorization", `Bearer ${ownerToken}`);

    expect(res.status).toBe(204);
  });

  it("owner TIDAK BISA dihapus lewat endpoint member", async () => {
    const res = await request(app)
      .delete(`/api/v1/vaults/${vaultId}/members/${ownerId}`)
      .set("Authorization", `Bearer ${ownerToken}`);

    expect(res.status).toBe(403);
  });

  it("semua endpoint di modul ini butuh login", async () => {
    const res = await request(app).get("/api/v1/vaults");
    expect(res.status).toBe(401);
  });
});
