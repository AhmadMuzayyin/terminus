// Logic bisnis murni — TIDAK IMPORT apa pun dari Express (lihat aturan
// konvensi DESIGN.md bagian 3). Otorisasi "apakah requester anggota/
// owner vault ini" SUDAH ditangani middleware (requireVaultMember/
// requireVaultOwner, lihat vaults.routes.ts) SEBELUM sampai ke sini —
// fungsi di bawah anggap requester SUDAH lolos cek itu.

import { prisma } from "../../db/client.js";
import { ConflictError, ForbiddenError, NotFoundError } from "../../errors.js";

interface VaultSummary {
  id: string;
  name: string;
  role: "owner" | "member";
}

// Bikin vault BARU + langsung jadikan pembuatnya owner, dalam satu
// nested-write Prisma (atomik — tidak mungkin ada Vault tanpa
// VaultMember owner-nya, atau sebaliknya).
export async function createVault(ownerUserId: string, name: string): Promise<VaultSummary> {
  const vault = await prisma.vault.create({
    data: {
      name,
      ownerUserId,
      members: { create: { userId: ownerUserId, role: "owner" } },
    },
  });
  return { id: vault.id, name: vault.name, role: "owner" };
}

export async function listVaultsForUser(userId: string): Promise<VaultSummary[]> {
  const memberships = await prisma.vaultMember.findMany({
    where: { userId },
    include: { vault: true },
  });
  return memberships.map((m) => ({ id: m.vault.id, name: m.vault.name, role: m.role }));
}

// Tambah member BY EMAIL user yang SUDAH TERDAFTAR — invite user yang
// belum punya akun sengaja BELUM didukung (lihat DESIGN.md bagian 7).
export async function addMember(
  vaultId: string,
  email: string,
): Promise<{ id: string; email: string; role: "member" }> {
  const user = await prisma.user.findUnique({ where: { email } });
  if (!user) {
    throw new NotFoundError(
      "User dengan email itu belum terdaftar di server ini — user harus punya akun dulu sebelum bisa ditambahkan ke vault.",
    );
  }

  const existing = await prisma.vaultMember.findUnique({
    where: { vaultId_userId: { vaultId, userId: user.id } },
  });
  if (existing) {
    throw new ConflictError("User ini sudah jadi anggota vault ini");
  }

  await prisma.vaultMember.create({ data: { vaultId, userId: user.id, role: "member" } });
  return { id: user.id, email: user.email, role: "member" };
}

// Owner TIDAK BISA dicabut lewat endpoint ini (v1 cuma punya satu
// owner per vault, mencabutnya bikin vault "yatim" tanpa siapa pun
// yang boleh kelola member) — pencabutan/transfer ownership di luar
// scope milestone ini.
export async function removeMember(vaultId: string, targetUserId: string): Promise<void> {
  const membership = await prisma.vaultMember.findUnique({
    where: { vaultId_userId: { vaultId, userId: targetUserId } },
  });
  if (!membership) {
    throw new NotFoundError("User itu bukan anggota vault ini");
  }
  if (membership.role === "owner") {
    throw new ForbiddenError("Owner vault tidak bisa dicabut lewat endpoint ini");
  }

  await prisma.vaultMember.delete({ where: { vaultId_userId: { vaultId, userId: targetUserId } } });
}
