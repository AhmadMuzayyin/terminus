// Error handler TERPUSAT — dipasang PALING TERAKHIR di app.ts (setelah
// semua route). `HttpError` (lihat src/errors.ts) diubah jadi response
// JSON sesuai status code-nya; error LAIN (bug tak terduga) di-log ke
// console DAN dibungkus jadi 500 generic — pesan aslinya TIDAK PERNAH
// bocor ke client (bisa saja isinya detail internal/query DB).

import type { ErrorRequestHandler } from "express";

import { HttpError } from "../errors.js";

export const errorHandler: ErrorRequestHandler = (err, _req, res, _next) => {
  if (err instanceof HttpError) {
    res.status(err.status).json({ error: err.message });
    return;
  }

  // eslint-disable-next-line no-console -- satu-satunya tempat error tak terduga dilaporkan
  console.error("Unhandled error:", err);
  res.status(500).json({ error: "Terjadi kesalahan internal server" });
};
