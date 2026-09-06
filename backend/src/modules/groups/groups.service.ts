// Logic bisnis murni — TIDAK IMPORT apa pun dari Express (lihat aturan
// konvensi DESIGN.md bagian 3).

import { prisma } from "../../db/client.js";
import { NotFoundError } from "../../errors.js";

interface GroupInput {
  // Lihat komentar `id` di hosts.schema.ts.
  id?: string;
  name: string;
  subtitle?: string | null;
  parentId?: string | null;
}

async function findGroupOrThrow(vaultId: string, id: string) {
  const group = await prisma.hostGroup.findUnique({ where: { id } });
  if (!group || group.vaultId !== vaultId) {
    throw new NotFoundError("Grup tidak ditemukan di vault ini");
  }
  return group;
}

export async function listGroups(vaultId: string) {
  return prisma.hostGroup.findMany({ where: { vaultId } });
}

export async function getGroup(vaultId: string, id: string) {
  return findGroupOrThrow(vaultId, id);
}

export async function createGroup(vaultId: string, input: GroupInput) {
  return prisma.hostGroup.create({
    data: {
      id: input.id,
      vaultId,
      name: input.name,
      subtitle: input.subtitle ?? null,
      parentId: input.parentId ?? null,
    },
  });
}

export async function updateGroup(vaultId: string, id: string, input: Partial<GroupInput>) {
  await findGroupOrThrow(vaultId, id);
  return prisma.hostGroup.update({
    where: { id },
    data: {
      ...(input.name !== undefined && { name: input.name }),
      ...(input.subtitle !== undefined && { subtitle: input.subtitle }),
      ...(input.parentId !== undefined && { parentId: input.parentId }),
    },
  });
}

// Hapus grup BESERTA semua host di dalamnya (cascade) — TERMASUK
// password terenkripsi tiap host itu. SENGAJA MIRROR PERSIS perilaku
// `terminus_vault::store::VaultStore::delete_group` di desktop app
// (lihat komentar panjang di sana soal ini keputusan destruktif atas
// permintaan eksplisit user) — client WAJIB konfirmasi dulu ke
// pemakainya sebelum manggil endpoint ini, sama seperti dialog
// konfirmasi di desktop app.
export async function deleteGroup(vaultId: string, id: string): Promise<void> {
  await findGroupOrThrow(vaultId, id);

  await prisma.$transaction(async (tx) => {
    const hosts = await tx.hostProfile.findMany({ where: { groupId: id }, select: { credentialId: true } });
    const credentialIds = hosts.map((h) => h.credentialId).filter((c): c is string => c !== null);
    if (credentialIds.length > 0) {
      await tx.secret.deleteMany({ where: { credentialId: { in: credentialIds } } });
    }
    await tx.hostProfile.deleteMany({ where: { groupId: id } });
    await tx.hostGroup.delete({ where: { id } });
  });
}
