// Baca + validasi environment variable SEKALI di sini (pakai zod) —
// modul lain import `config` dari sini, JANGAN baca `process.env`
// langsung di tempat lain. Kalau env var wajib hilang/salah format,
// proses gagal start dengan pesan jelas (bukan error random di tengah
// jalan waktu env itu baru kepakai).

import { z } from "zod";

const envSchema = z.object({
  DATABASE_URL: z.string().min(1, "DATABASE_URL wajib diisi"),
  PORT: z.coerce.number().int().positive().default(4000),
  // Base64 32 byte (256-bit) — divalidasi panjang HASIL decode-nya,
  // bukan panjang string base64-nya sendiri (beda karena encoding).
  SERVER_MASTER_KEY: z
    .string()
    .min(1, "SERVER_MASTER_KEY wajib diisi")
    .refine((v) => Buffer.from(v, "base64").length === 32, {
      message: "SERVER_MASTER_KEY harus base64 dari 32 byte (256-bit) random",
    }),
  JWT_SECRET: z.string().min(16, "JWT_SECRET minimal 16 karakter"),
  ACCESS_TOKEN_TTL_SECONDS: z.coerce.number().int().positive().default(900),
  REFRESH_TOKEN_TTL_SECONDS: z.coerce.number().int().positive().default(2592000),
});

const parsed = envSchema.safeParse(process.env);
if (!parsed.success) {
  // eslint-disable-next-line no-console -- proses belum sempat setup logger, dan ini fatal (proses berhenti)
  console.error("Konfigurasi environment tidak valid:", parsed.error.flatten().fieldErrors);
  process.exit(1);
}

export const config = parsed.data;
