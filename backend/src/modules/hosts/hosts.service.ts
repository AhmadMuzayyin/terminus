// Logic bisnis murni — TIDAK IMPORT apa pun dari Express (lihat aturan
// konvensi DESIGN.md bagian 3).

import { randomUUID } from "node:crypto";

import { decryptSecret, encryptSecret } from "../../crypto/secrets.js";
import { prisma } from "../../db/client.js";
import { NotFoundError } from "../../errors.js";

export interface HostInput {
  label: string;
  host: string;
  port: number;
  username: string;
  kind: string;
  groupId?: string | null;
  tags?: string[];
  terminalTheme?: string | null;
}

export interface HostResponse {
  id: string;
  label: string;
  host: string;
  port: number;
  username: string;
  kind: string;
  groupId: string | null;
  tags: string[];
  terminalTheme: string | null;
  // Password itu SENDIRI TIDAK PERNAH ikut di sini (lihat DESIGN.md
  // bagian 5.5) — cuma indikator ADA/TIDAKnya, buat UI (mis. "isi
  // password dulu sebelum connect", sama pola dengan notice di panel
  // desktop app buat host hasil Import SecureCRT).
  hasPassword: boolean;
}

interface HostRow {
  id: string;
  label: string;
  host: string;
  port: number;
  username: string;
  kind: string;
  groupId: string | null;
  tags: string;
  terminalTheme: string | null;
  credentialId: string | null;
}

function toResponse(row: HostRow, secretCredentialIds: ReadonlySet<string>): HostResponse {
  return {
    id: row.id,
    label: row.label,
    host: row.host,
    port: row.port,
    username: row.username,
    kind: row.kind,
    groupId: row.groupId,
    tags: JSON.parse(row.tags) as string[],
    terminalTheme: row.terminalTheme,
    hasPassword: row.credentialId !== null && secretCredentialIds.has(row.credentialId),
  };
}

// Satu query buat SEMUA host di list, bukan N+1 per host — kumpulkan
// dulu credentialId mana saja yang beneran punya baris `secrets`.
async function loadSecretCredentialIds(credentialIds: (string | null)[]): Promise<Set<string>> {
  const ids = credentialIds.filter((c): c is string => c !== null);
  if (ids.length === 0) {
    return new Set();
  }
  const secrets = await prisma.secret.findMany({
    where: { credentialId: { in: ids } },
    select: { credentialId: true },
  });
  return new Set(secrets.map((s) => s.credentialId));
}

async function findHostOrThrow(vaultId: string, id: string): Promise<HostRow> {
  const host = await prisma.hostProfile.findUnique({ where: { id } });
  if (!host || host.vaultId !== vaultId) {
    throw new NotFoundError("Host tidak ditemukan di vault ini");
  }
  return host;
}

export async function listHosts(vaultId: string): Promise<HostResponse[]> {
  const hosts = await prisma.hostProfile.findMany({ where: { vaultId } });
  const secretIds = await loadSecretCredentialIds(hosts.map((h) => h.credentialId));
  return hosts.map((h) => toResponse(h, secretIds));
}

export async function getHost(vaultId: string, id: string): Promise<HostResponse> {
  const host = await findHostOrThrow(vaultId, id);
  const secretIds = await loadSecretCredentialIds([host.credentialId]);
  return toResponse(host, secretIds);
}

// Host BOLEH dibuat TANPA password (`credentialId` selalu digenerate,
// "menggantung" tanpa baris `secrets` sampai diisi lewat
// `PUT .../secret`) — mirror pola host hasil Import SecureCRT di
// desktop app, BUKAN state error.
export async function createHost(vaultId: string, input: HostInput): Promise<HostResponse> {
  const host = await prisma.hostProfile.create({
    data: {
      vaultId,
      label: input.label,
      host: input.host,
      port: input.port,
      username: input.username,
      kind: input.kind,
      authMethod: "password",
      credentialId: randomUUID(),
      groupId: input.groupId ?? null,
      tags: JSON.stringify(input.tags ?? []),
      terminalTheme: input.terminalTheme ?? null,
    },
  });
  return toResponse(host, new Set());
}

export async function updateHost(vaultId: string, id: string, input: Partial<HostInput>): Promise<HostResponse> {
  await findHostOrThrow(vaultId, id);
  const host = await prisma.hostProfile.update({
    where: { id },
    data: {
      ...(input.label !== undefined && { label: input.label }),
      ...(input.host !== undefined && { host: input.host }),
      ...(input.port !== undefined && { port: input.port }),
      ...(input.username !== undefined && { username: input.username }),
      ...(input.kind !== undefined && { kind: input.kind }),
      ...(input.groupId !== undefined && { groupId: input.groupId }),
      ...(input.tags !== undefined && { tags: JSON.stringify(input.tags) }),
      ...(input.terminalTheme !== undefined && { terminalTheme: input.terminalTheme }),
    },
  });
  const secretIds = await loadSecretCredentialIds([host.credentialId]);
  return toResponse(host, secretIds);
}

export async function deleteHost(vaultId: string, id: string): Promise<void> {
  const host = await findHostOrThrow(vaultId, id);
  await prisma.$transaction(async (tx) => {
    if (host.credentialId) {
      await tx.secret.deleteMany({ where: { credentialId: host.credentialId } });
    }
    await tx.hostProfile.delete({ where: { id } });
  });
}

export async function setSecret(vaultId: string, id: string, password: string): Promise<void> {
  const host = await findHostOrThrow(vaultId, id);
  if (!host.credentialId) {
    // Tidak seharusnya kejadian — createHost SELALU generate
    // credentialId. Jaga-jaga eksplisit daripada nulis ke kolom NULL
    // diam-diam gagal di level DB.
    throw new NotFoundError("Host ini tidak punya credentialId — data tidak konsisten");
  }

  const encrypted = encryptSecret(password);
  await prisma.secret.upsert({
    where: { credentialId: host.credentialId },
    create: { credentialId: host.credentialId, vaultId, encryptedData: encrypted },
    update: { encryptedData: encrypted },
  });
}

export async function getSecret(vaultId: string, id: string): Promise<string> {
  const host = await findHostOrThrow(vaultId, id);
  const secret = host.credentialId
    ? await prisma.secret.findUnique({ where: { credentialId: host.credentialId } })
    : null;
  if (!secret) {
    throw new NotFoundError("Host ini belum punya password tersimpan");
  }
  return decryptSecret(secret.encryptedData);
}
