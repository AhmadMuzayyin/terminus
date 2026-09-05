// Augmentasi Express.Request nambah field `user` (diisi middleware
// `requireAuth`, lihat src/middleware/auth.ts) DAN `vaultRole` (diisi
// `requireVaultMember`, lihat src/middleware/vaultAccess.ts) — supaya
// controller bisa akses keduanya type-safe, tanpa `as any`.

import "express";

import type { VaultRole } from "@prisma/client";

declare global {
  namespace Express {
    interface Request {
      user?: {
        id: string;
      };
      vaultRole?: VaultRole;
    }
  }
}
