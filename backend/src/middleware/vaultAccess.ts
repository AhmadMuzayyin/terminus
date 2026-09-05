// requireVaultMember: cek `req.user` (diisi requireAuth SEBELUM ini,
// wajib dipasang duluan) beneran anggota `:vaultId` di URL param, isi
// `req.vaultRole` kalau iya. requireVaultOwner: cek `req.vaultRole`
// (diisi requireVaultMember SEBELUM ini) adalah "owner" — dipakai
// route yang cuma boleh owner (tambah/hapus member).
// validateVaultIdParam: cek format `:vaultId` valid UUID SEBELUM
// requireVaultMember sempat query DB dengannya — tanpa ini, vaultId
// asal-asalan (mis. "bukan-uuid") jatuh ke requireVaultMember dulu,
// yang cuma bakal bilang "bukan anggota" (403, technically benar tapi
// membingungkan) alih-alih "format tidak valid" (400) yang lebih jelas.
// DITEMUKAN lewat test hosts.test.ts waktu Milestone 4 dikerjakan —
// hosts/groups/identities di-mount pakai requireVaultMember di level
// app.use() (SEBELUM `validate()` per-route di masing-masing router),
// beda dari modul vaults yang validate()-nya jalan duluan per-route.

import type { NextFunction, Request, Response } from "express";
import { z } from "zod";

import { prisma } from "../db/client.js";
import { BadRequestError, ForbiddenError, UnauthorizedError } from "../errors.js";

const vaultIdSchema = z.string().uuid();

export function validateVaultIdParam(req: Request, _res: Response, next: NextFunction) {
  const result = vaultIdSchema.safeParse(req.params.vaultId);
  if (!result.success) {
    next(new BadRequestError("vaultId tidak valid"));
    return;
  }
  next();
}

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
