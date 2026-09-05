// Self-contained (lihat pola sama di vaults.test.ts/hosts.test.ts).

import { afterAll, beforeAll, describe, expect, it } from "vitest";
import request from "supertest";

import { createApp } from "../src/app.js";
import { hashPassword } from "../src/crypto/password.js";
import { signAccessToken } from "../src/crypto/tokens.js";
import { prisma } from "../src/db/client.js";

const app = createApp();
const OWNER_EMAIL = "owner-identities-test@terminus.test";

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
      name: "Vault Identities Test",
      ownerUserId: ownerId,
      members: { create: { userId: ownerId, role: "owner" } },
    },
  });
  vaultId = vault.id;
});

afterAll(async () => {
  await prisma.vault.deleteMany({ where: { id: vaultId } });
  await prisma.user.deleteMany({ where: { id: ownerId } });
});

describe("Identities CRUD + secret", () => {
  let identityId = "";

  it("create identity TANPA password ditolak validasi (400) — Identity SELALU punya password", async () => {
    const res = await request(app)
      .post(`/api/v1/vaults/${vaultId}/identities`)
      .set("Authorization", `Bearer ${ownerToken}`)
      .send({ label: "NOC Router" });

    expect(res.status).toBe(400);
  });

  it("create identity lengkap berhasil, password TIDAK PERNAH ikut di response", async () => {
    const res = await request(app)
      .post(`/api/v1/vaults/${vaultId}/identities`)
      .set("Authorization", `Bearer ${ownerToken}`)
      .send({ label: "NOC Router", username: "bro-noc", password: "secret-identity-pw" });

    expect(res.status).toBe(201);
    expect(res.body).toEqual({ id: expect.any(String), label: "NOC Router", username: "bro-noc" });
    identityId = res.body.id;
  });

  it("get identity by id", async () => {
    const res = await request(app)
      .get(`/api/v1/vaults/${vaultId}/identities/${identityId}`)
      .set("Authorization", `Bearer ${ownerToken}`);

    expect(res.status).toBe(200);
    expect(res.body.username).toBe("bro-noc");
  });

  it("get secret identity balikin password plaintext yang SAMA", async () => {
    const res = await request(app)
      .get(`/api/v1/vaults/${vaultId}/identities/${identityId}/secret`)
      .set("Authorization", `Bearer ${ownerToken}`);

    expect(res.status).toBe(200);
    expect(res.body.password).toBe("secret-identity-pw");
  });

  it("list identities berisi identity yang baru dibuat", async () => {
    const res = await request(app)
      .get(`/api/v1/vaults/${vaultId}/identities`)
      .set("Authorization", `Bearer ${ownerToken}`);

    expect(res.status).toBe(200);
    expect(res.body.some((i: { id: string }) => i.id === identityId)).toBe(true);
  });

  it("update identity ganti username TANPA sertakan password -> password lama TETAP", async () => {
    const res = await request(app)
      .put(`/api/v1/vaults/${vaultId}/identities/${identityId}`)
      .set("Authorization", `Bearer ${ownerToken}`)
      .send({ username: "bro-noc-baru" });

    expect(res.status).toBe(200);
    expect(res.body.username).toBe("bro-noc-baru");

    const secretRes = await request(app)
      .get(`/api/v1/vaults/${vaultId}/identities/${identityId}/secret`)
      .set("Authorization", `Bearer ${ownerToken}`);
    expect(secretRes.body.password).toBe("secret-identity-pw");
  });

  it("update identity DENGAN password baru -> password ikut berubah", async () => {
    await request(app)
      .put(`/api/v1/vaults/${vaultId}/identities/${identityId}`)
      .set("Authorization", `Bearer ${ownerToken}`)
      .send({ password: "password-identity-baru" });

    const secretRes = await request(app)
      .get(`/api/v1/vaults/${vaultId}/identities/${identityId}/secret`)
      .set("Authorization", `Bearer ${ownerToken}`);
    expect(secretRes.body.password).toBe("password-identity-baru");
  });

  it("user yang bukan anggota vault ditolak (403)", async () => {
    const outsider = await prisma.user.create({
      data: {
        email: "outsider-identities-test@terminus.test",
        passwordHash: await hashPassword("irrelevant-pw-123"),
      },
    });
    const outsiderToken = signAccessToken(outsider.id);

    const res = await request(app)
      .get(`/api/v1/vaults/${vaultId}/identities`)
      .set("Authorization", `Bearer ${outsiderToken}`);
    expect(res.status).toBe(403);

    await prisma.user.delete({ where: { id: outsider.id } });
  });

  it("hapus identity BESERTA password-nya", async () => {
    const res = await request(app)
      .delete(`/api/v1/vaults/${vaultId}/identities/${identityId}`)
      .set("Authorization", `Bearer ${ownerToken}`);
    expect(res.status).toBe(204);

    const getRes = await request(app)
      .get(`/api/v1/vaults/${vaultId}/identities/${identityId}`)
      .set("Authorization", `Bearer ${ownerToken}`);
    expect(getRes.status).toBe(404);
  });
});
