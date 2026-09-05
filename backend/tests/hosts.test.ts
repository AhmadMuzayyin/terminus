// Self-contained (lihat pola sama di vaults.test.ts): bikin user+vault
// sendiri lewat Prisma langsung, bersihkan penuh di afterAll (Vault
// dihapus DULU baru User-nya — Vault.owner_user_id tidak cascade).

import { afterAll, beforeAll, describe, expect, it } from "vitest";
import request from "supertest";

import { createApp } from "../src/app.js";
import { hashPassword } from "../src/crypto/password.js";
import { signAccessToken } from "../src/crypto/tokens.js";
import { prisma } from "../src/db/client.js";

const app = createApp();
const OWNER_EMAIL = "owner-hosts-test@terminus.test";

let ownerId = "";
let ownerToken = "";
let vaultId = "";

beforeAll(async () => {
  await prisma.vault.deleteMany({ where: { owner: { email: OWNER_EMAIL } } });
  await prisma.user.deleteMany({ where: { email: OWNER_EMAIL } });

  const owner = await prisma.user.create({
    data: { email: OWNER_EMAIL, passwordHash: await hashPassword("owner-password-123") },
  });
  ownerId = owner.id;
  ownerToken = signAccessToken(ownerId);

  const vault = await prisma.vault.create({
    data: {
      name: "Vault Hosts Test",
      ownerUserId: ownerId,
      members: { create: { userId: ownerId, role: "owner" } },
    },
  });
  vaultId = vault.id;
});

afterAll(async () => {
  await prisma.vault.deleteMany({ where: { id: vaultId } }); // cascade: hosts + secrets + members
  await prisma.user.deleteMany({ where: { id: ownerId } });
});

describe("Hosts CRUD + secret", () => {
  let hostId = "";

  it("list hosts awalnya kosong", async () => {
    const res = await request(app)
      .get(`/api/v1/vaults/${vaultId}/hosts`)
      .set("Authorization", `Bearer ${ownerToken}`);

    expect(res.status).toBe(200);
    expect(res.body).toEqual([]);
  });

  it("create host TANPA password -> hasPassword false (mirror host hasil Import SecureCRT)", async () => {
    const res = await request(app)
      .post(`/api/v1/vaults/${vaultId}/hosts`)
      .set("Authorization", `Bearer ${ownerToken}`)
      .send({ label: "prod-web-01", host: "10.0.0.5", username: "deploy" });

    expect(res.status).toBe(201);
    expect(res.body).toMatchObject({
      label: "prod-web-01",
      host: "10.0.0.5",
      port: 22,
      username: "deploy",
      kind: "ssh",
      groupId: null,
      tags: [],
      terminalTheme: null,
      hasPassword: false,
    });
    hostId = res.body.id;
  });

  it("get host by id balikin data yang sama", async () => {
    const res = await request(app)
      .get(`/api/v1/vaults/${vaultId}/hosts/${hostId}`)
      .set("Authorization", `Bearer ${ownerToken}`);

    expect(res.status).toBe(200);
    expect(res.body.id).toBe(hostId);
  });

  it("list hosts sekarang berisi host yang baru dibuat", async () => {
    const res = await request(app)
      .get(`/api/v1/vaults/${vaultId}/hosts`)
      .set("Authorization", `Bearer ${ownerToken}`);

    expect(res.status).toBe(200);
    expect(res.body).toHaveLength(1);
  });

  it("update host mengubah field yang dikirim, field lain tetap utuh", async () => {
    const res = await request(app)
      .put(`/api/v1/vaults/${vaultId}/hosts/${hostId}`)
      .set("Authorization", `Bearer ${ownerToken}`)
      .send({ port: 2222, tags: ["production", "database"] });

    expect(res.status).toBe(200);
    expect(res.body.port).toBe(2222);
    expect(res.body.tags).toEqual(["production", "database"]);
    expect(res.body.label).toBe("prod-web-01");
  });

  it("get secret SEBELUM diisi ditolak (404)", async () => {
    const res = await request(app)
      .get(`/api/v1/vaults/${vaultId}/hosts/${hostId}/secret`)
      .set("Authorization", `Bearer ${ownerToken}`);

    expect(res.status).toBe(404);
  });

  it("set secret berhasil -> hasPassword jadi true", async () => {
    const setRes = await request(app)
      .put(`/api/v1/vaults/${vaultId}/hosts/${hostId}/secret`)
      .set("Authorization", `Bearer ${ownerToken}`)
      .send({ password: "password-ssh-rahasia" });
    expect(setRes.status).toBe(204);

    const getHostRes = await request(app)
      .get(`/api/v1/vaults/${vaultId}/hosts/${hostId}`)
      .set("Authorization", `Bearer ${ownerToken}`);
    expect(getHostRes.body.hasPassword).toBe(true);
  });

  it("get secret setelah diisi balikin plaintext yang SAMA", async () => {
    const res = await request(app)
      .get(`/api/v1/vaults/${vaultId}/hosts/${hostId}/secret`)
      .set("Authorization", `Bearer ${ownerToken}`);

    expect(res.status).toBe(200);
    expect(res.body.password).toBe("password-ssh-rahasia");
  });

  it("set secret ULANG (replace) -> baca balik dapat nilai yang BARU", async () => {
    await request(app)
      .put(`/api/v1/vaults/${vaultId}/hosts/${hostId}/secret`)
      .set("Authorization", `Bearer ${ownerToken}`)
      .send({ password: "password-baru-lagi" });

    const res = await request(app)
      .get(`/api/v1/vaults/${vaultId}/hosts/${hostId}/secret`)
      .set("Authorization", `Bearer ${ownerToken}`);
    expect(res.body.password).toBe("password-baru-lagi");
  });

  it("host di vault LAIN tidak bisa diakses lewat vaultId ini (404, isolasi antar vault)", async () => {
    const otherVault = await prisma.vault.create({
      data: {
        name: "Vault Lain",
        ownerUserId: ownerId,
        members: { create: { userId: ownerId, role: "owner" } },
      },
    });

    const res = await request(app)
      .get(`/api/v1/vaults/${otherVault.id}/hosts/${hostId}`)
      .set("Authorization", `Bearer ${ownerToken}`);
    expect(res.status).toBe(404);

    await prisma.vault.delete({ where: { id: otherVault.id } });
  });

  it("user yang bukan anggota vault ditolak (403)", async () => {
    const outsider = await prisma.user.create({
      data: {
        email: "outsider-hosts-test@terminus.test",
        passwordHash: await hashPassword("irrelevant-pw-123"),
      },
    });
    const outsiderToken = signAccessToken(outsider.id);

    const res = await request(app)
      .get(`/api/v1/vaults/${vaultId}/hosts`)
      .set("Authorization", `Bearer ${outsiderToken}`);
    expect(res.status).toBe(403);

    await prisma.user.delete({ where: { id: outsider.id } });
  });

  it("vaultId dengan format bukan UUID ditolak validasi (400)", async () => {
    const res = await request(app)
      .get("/api/v1/vaults/bukan-uuid/hosts")
      .set("Authorization", `Bearer ${ownerToken}`);
    expect(res.status).toBe(400);
  });

  it("delete host berhasil, secret ikut kehapus", async () => {
    const res = await request(app)
      .delete(`/api/v1/vaults/${vaultId}/hosts/${hostId}`)
      .set("Authorization", `Bearer ${ownerToken}`);
    expect(res.status).toBe(204);

    const getRes = await request(app)
      .get(`/api/v1/vaults/${vaultId}/hosts/${hostId}`)
      .set("Authorization", `Bearer ${ownerToken}`);
    expect(getRes.status).toBe(404);
  });
});
