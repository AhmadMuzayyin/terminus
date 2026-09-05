// requireAuth: verifikasi JWT access token dari header
// `Authorization: Bearer <token>`, isi `req.user` (lihat augmentasi
// tipe di src/types/express.d.ts) kalau valid. Dipasang di route mana
// pun yang butuh login — termasuk NANTI `requireVaultMember` (modul
// vaults, belum dikerjakan) yang jalan SETELAH middleware ini.

import type { NextFunction, Request, Response } from "express";

import { verifyAccessToken } from "../crypto/tokens.js";
import { UnauthorizedError } from "../errors.js";

const BEARER_PREFIX = "Bearer ";

export function requireAuth(req: Request, _res: Response, next: NextFunction) {
  const header = req.header("authorization");
  const token = header?.startsWith(BEARER_PREFIX) ? header.slice(BEARER_PREFIX.length) : undefined;

  if (!token) {
    next(new UnauthorizedError("Header Authorization: Bearer <token> wajib diisi"));
    return;
  }

  try {
    const payload = verifyAccessToken(token);
    req.user = { id: payload.sub };
    next();
  } catch {
    next(new UnauthorizedError("Access token tidak valid atau sudah kedaluwarsa"));
  }
}
