// Express 4 TIDAK menangkap otomatis Promise yang reject dari dalam
// route handler async — tanpa ini, error di handler async bakal jadi
// "unhandled rejection" diam-diam, TIDAK PERNAH nyampe ke
// `errorHandler`. Bungkus tiap handler async pakai ini di
// `*.controller.ts` supaya rejection-nya diteruskan ke `next(err)`.

import type { NextFunction, Request, RequestHandler, Response } from "express";

export function asyncHandler(
  handler: (req: Request, res: Response, next: NextFunction) => Promise<void>,
): RequestHandler {
  return (req, res, next) => {
    handler(req, res, next).catch(next);
  };
}
