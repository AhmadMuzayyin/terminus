// Augmentasi Express.Request nambah field `user` (diisi middleware
// `requireAuth`, lihat src/middleware/auth.ts) — supaya controller
// bisa akses `req.user.id` dengan type-safe, tanpa `as any`.

import "express";

declare global {
  namespace Express {
    interface Request {
      user?: {
        id: string;
      };
    }
  }
}
