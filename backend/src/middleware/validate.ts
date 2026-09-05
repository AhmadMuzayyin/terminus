// Bungkus skema zod (bentuk `{ body, query, params }`, lihat contoh di
// `modules/auth/auth.schema.ts`) jadi middleware Express — dipasang di
// `*.routes.ts` SEBELUM controller. Gagal validasi -> `BadRequestError`
// (400) lewat `errorHandler`, controller-nya sendiri tidak pernah lihat
// input yang belum tervalidasi.

import type { NextFunction, Request, Response } from "express";
import type { ZodTypeAny } from "zod";

import { BadRequestError } from "../errors.js";

export function validate(schema: ZodTypeAny) {
  return (req: Request, _res: Response, next: NextFunction) => {
    const result = schema.safeParse({ body: req.body, query: req.query, params: req.params });
    if (!result.success) {
      next(new BadRequestError(result.error.issues.map((issue) => issue.message).join("; ")));
      return;
    }

    // Timpa `req.body` dengan hasil parse zod (bukan input mentah) —
    // biar default value/coercion skema (mis. `z.coerce.number()`)
    // beneran kepakai di controller, bukan cuma buat validasi doang.
    const parsed = result.data as { body?: unknown };
    if (parsed.body !== undefined) {
      req.body = parsed.body;
    }
    next();
  };
}
