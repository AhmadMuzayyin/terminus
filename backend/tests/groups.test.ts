// Self-contained (lihat pola sama di vaults.test.ts/hosts.test.ts).

import { randomUUID } from "node:crypto";

import { afterAll, beforeAll, describe, expect, it } from "vitest";
import request from "supertest";

import { createApp } from "../src/app.js";
import { hashPassword } from "../src/crypto/password.js";
import { signAccessToken } from "../src/crypto/tokens.js";
import { prisma } from "../src/db/client.js";

const app = createApp();
const OWNER_EMAIL = "owner-groups-test@terminus.test";

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
      name: "Vault Groups Test",
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

describe("Groups CRUD + cascade delete", () => {
  let groupId = "";

  it("create grup baru", async () => {
    const res = await request(app)
      .post(`/api/v1/vaults/${vaultId}/groups`)
      .set("Authorization", `Bearer ${ownerToken}`)
      .send({ name: "ROUTER" });

    expect(res.status).toBe(201);
    expect(res.body.name).toBe("ROUTER");
    groupId = res.body.id;
  });

  it("create grup TANPA nama ditolak validasi (400)", async () => {
    const res = await request(app)
      .post(`/api/v1/vaults/${vaultId}/groups`)
      .set("Authorization", `Bearer ${ownerToken}`)
      .send({});
    expect(res.status).toBe(400);
  });

  it("create grup DENGAN id yang dikirim client -> id itu dipakai apa adanya", async () => {
    const clientId = randomUUID();
    const res = await request(app)
      .post(`/api/v1/vaults/${vaultId}/groups`)
      .set("Authorization", `Bearer ${ownerToken}`)
      .send({ id: clientId, name: "SWITCH" });

    expect(res.status).toBe(201);
    expect(res.body.id).toBe(clientId);
  });

  it("get grup by id", async () => {
    const res = await request(app)
      .get(`/api/v1/vaults/${vaultId}/groups/${groupId}`)
      .set("Authorization", `Bearer ${ownerToken}`);

    expect(res.status).toBe(200);
    expect(res.body.id).toBe(groupId);
  });

  it("list grup berisi grup yang baru dibuat", async () => {
    const res = await request(app)
      .get(`/api/v1/vaults/${vaultId}/groups`)
      .set("Authorization", `Bearer ${ownerToken}`);

    expect(res.status).toBe(200);
    expect(res.body.some((g: { id: string }) => g.id === groupId)).toBe(true);
  });

  it("update grup", async () => {
    const res = await request(app)
      .put(`/api/v1/vaults/${vaultId}/groups/${groupId}`)
      .set("Authorization", `Bearer ${ownerToken}`)
      .send({ subtitle: "Jakarta" });

    expect(res.status).toBe(200);
    expect(res.body.subtitle).toBe("Jakarta");
    expect(res.body.name).toBe("ROUTER");
  });

  it("hapus grup MENCABUT host di dalamnya BESERTA password-nya (cascade, mirror desktop app)", async () => {
    const hostRes = await request(app)
      .post(`/api/v1/vaults/${vaultId}/hosts`)
      .set("Authorization", `Bearer ${ownerToken}`)
      .send({ label: "rtr-01", host: "10.1.1.1", username: "admin", groupId });
    const hostId: string = hostRes.body.id;

    await request(app)
      .put(`/api/v1/vaults/${vaultId}/hosts/${hostId}/secret`)
      .set("Authorization", `Bearer ${ownerToken}`)
      .send({ password: "password-router" });

    const deleteRes = await request(app)
      .delete(`/api/v1/vaults/${vaultId}/groups/${groupId}`)
      .set("Authorization", `Bearer ${ownerToken}`);
    expect(deleteRes.status).toBe(204);

    const groupAfter = await request(app)
      .get(`/api/v1/vaults/${vaultId}/groups/${groupId}`)
      .set("Authorization", `Bearer ${ownerToken}`);
    expect(groupAfter.status).toBe(404);

    const hostAfter = await request(app)
      .get(`/api/v1/vaults/${vaultId}/hosts/${hostId}`)
      .set("Authorization", `Bearer ${ownerToken}`);
    expect(hostAfter.status).toBe(404);
  });

  it("user yang bukan anggota vault ditolak (403)", async () => {
    const outsider = await prisma.user.create({
      data: {
        email: "outsider-groups-test@terminus.test",
        passwordHash: await hashPassword("irrelevant-pw-123"),
      },
    });
    const outsiderToken = signAccessToken(outsider.id);

    const res = await request(app)
      .get(`/api/v1/vaults/${vaultId}/groups`)
      .set("Authorization", `Bearer ${outsiderToken}`);
    expect(res.status).toBe(403);

    await prisma.user.delete({ where: { id: outsider.id } });
  });
});
