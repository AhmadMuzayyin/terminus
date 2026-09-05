// requireVaultMember: cek `req.user` (diisi requireAuth SEBELUM ini,
// wajib dipasang duluan) beneran anggota `:vaultId` di URL param, isi
// `req.vaultRole` kalau iya. requireVaultOwner: cek `req.vaultRole`
// (diisi requireVaultMember SEBELUM ini) adalah "owner" — dipakai
// route yang cuma boleh owner (tambah/hapus member).

import type { NextFunction, Request, Response } from "express";

import { prisma } from "../db/client.js";
import { ForbiddenError, UnauthorizedError } from "../errors.js";

export async function requireVaultMember(req: Request, _res: Response, next: NextFunction) {
  try {
    if (!req.user) {
      // Tidak seharusnya kejadian (requireAuth selalu dipasang
      // duluan) — jaga-jaga eksplisit daripada `req.user!` diam-diam
      // salah kalau urutan middleware kebalik suatu saat.
      throw new UnauthorizedError();
    }

    const { vaultId } = req.params;
    const membership = await prisma.vaultMember.findUnique({
      where: { vaultId_userId: { vaultId: vaultId!, userId: req.user.id } },
    });
    if (!membership) {
      throw new ForbiddenError("Anda bukan anggota vault ini");
    }

    req.vaultRole = membership.role;
    next();
  } catch (error) {
    next(error);
  }
}

export function requireVaultOwner(req: Request, _res: Response, next: NextFunction) {
  if (req.vaultRole !== "owner") {
    next(new ForbiddenError("Cuma owner vault yang boleh melakukan aksi ini"));
    return;
  }
  next();
}
