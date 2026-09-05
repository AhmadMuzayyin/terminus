// Logic bisnis murni — TIDAK IMPORT apa pun dari Express (lihat aturan
// konvensi DESIGN.md bagian 3).
//
// BEDA dari host: Identity SELALU punya password sejak dibuat (tidak
// ada state "menggantung tanpa password") — mirror
// `terminus_core::Identity` + alur `identity-create-requested` desktop
// app, yang SELALU sertakan password dalam SATU langkah (bukan dua
// langkah seperti host: create dulu, isi password belakangan). DESIGN.md
// bagian 5.5 aslinya cuma mendetailkan endpoint secret buat host —
// identity punya password-nya sendiri langsung di body create/update,
// PLUS satu endpoint `GET .../secret` buat client yang perlu
// dekripsi-nya (dipakai waktu "pick identity" ngisi form host baru,
// mirror `on_identity_picked` di desktop app).

import { randomUUID } from "node:crypto";

import { decryptSecret, encryptSecret } from "../../crypto/secrets.js";
import { prisma } from "../../db/client.js";
import { NotFoundError } from "../../errors.js";

interface IdentityResponse {
  id: string;
  label: string;
  username: string;
}

function toResponse(row: { id: string; label: string; username: string }): IdentityResponse {
  return { id: row.id, label: row.label, username: row.username };
}

async function findIdentityOrThrow(vaultId: string, id: string) {
  const identity = await prisma.identity.findUnique({ where: { id } });
  if (!identity || identity.vaultId !== vaultId) {
    throw new NotFoundError("Identity tidak ditemukan di vault ini");
  }
  return identity;
}

export async function listIdentities(vaultId: string): Promise<IdentityResponse[]> {
  const identities = await prisma.identity.findMany({ where: { vaultId } });
  return identities.map(toResponse);
}

export async function getIdentity(vaultId: string, id: string): Promise<IdentityResponse> {
  return toResponse(await findIdentityOrThrow(vaultId, id));
}

export async function createIdentity(
  vaultId: string,
  input: { label: string; username: string; password: string },
): Promise<IdentityResponse> {
  const credentialId = randomUUID();
  const encrypted = encryptSecret(input.password);

  const identity = await prisma.$transaction(async (tx) => {
    await tx.secret.create({ data: { credentialId, vaultId, encryptedData: encrypted } });
    return tx.identity.create({ data: { vaultId, label: input.label, username: input.username, credentialId } });
  });

  return toResponse(identity);
}

export async function updateIdentity(
  vaultId: string,
  id: string,
  input: { label?: string; username?: string; password?: string },
): Promise<IdentityResponse> {
  const existing = await findIdentityOrThrow(vaultId, id);

  if (input.password !== undefined) {
    const encrypted = encryptSecret(input.password);
    await prisma.secret.upsert({
      where: { credentialId: existing.credentialId },
      create: { credentialId: existing.credentialId, vaultId, encryptedData: encrypted },
      update: { encryptedData: encrypted },
    });
  }

  const identity = await prisma.identity.update({
    where: { id },
    data: {
      ...(input.label !== undefined && { label: input.label }),
      ...(input.username !== undefined && { username: input.username }),
    },
  });

  return toResponse(identity);
}

export async function deleteIdentity(vaultId: string, id: string): Promise<void> {
  const identity = await findIdentityOrThrow(vaultId, id);
  await prisma.$transaction(async (tx) => {
    await tx.secret.deleteMany({ where: { credentialId: identity.credentialId } });
    await tx.identity.delete({ where: { id } });
  });
}

export async function getIdentitySecret(vaultId: string, id: string): Promise<string> {
  const identity = await findIdentityOrThrow(vaultId, id);
  const secret = await prisma.secret.findUnique({ where: { credentialId: identity.credentialId } });
  if (!secret) {
    // Tidak seharusnya kejadian — createIdentity SELALU sertakan
    // password. Jaga-jaga defensif kalau data korup/dihapus manual
    // lewat query langsung ke DB.
    throw new NotFoundError("Identity ini tidak punya password tersimpan (data tidak konsisten)");
  }
  return decryptSecret(secret.encryptedData);
}
