// Access token (JWT, stateless, umur pendek) + refresh token (random
// string, umur panjang, DISIMPAN TER-HASH di tabel refresh_tokens
// lewat auth.service.ts) — lihat DESIGN.md bagian 2 & 5.2 buat alasan
// kenapa dua mekanisme berbeda dipakai bareng (JWT stateless tidak
// bisa di-revoke, refresh token yang stateful di DB bisa).

import crypto from "node:crypto";

import jwt from "jsonwebtoken";

import { config } from "../config/env.js";

export interface AccessTokenPayload {
  sub: string; // user id
}

export function signAccessToken(userId: string): string {
  return jwt.sign({ sub: userId }, config.JWT_SECRET, { expiresIn: config.ACCESS_TOKEN_TTL_SECONDS });
}

// Lempar `jwt.JsonWebTokenError`/`TokenExpiredError` kalau tidak valid
// — caller (`middleware/auth.ts`) yang tangkap & ubah jadi 401.
export function verifyAccessToken(token: string): AccessTokenPayload {
  const decoded = jwt.verify(token, config.JWT_SECRET);
  if (typeof decoded === "string" || typeof decoded.sub !== "string") {
    throw new Error("Payload access token tidak sesuai skema yang diharapkan");
  }
  return { sub: decoded.sub };
}

const REFRESH_TOKEN_BYTES = 48;

// Random string base64url (bukan JWT) — nilai MENTAH ini yang dikirim
// ke client, TIDAK PERNAH disimpan mentah di database (lihat
// `hashRefreshToken` di bawah).
export function generateRefreshToken(): string {
  return crypto.randomBytes(REFRESH_TOKEN_BYTES).toString("base64url");
}

// SHA-256 cukup di sini (BEDA dari `hashPassword` yang pakai Argon2id
// lambat) — refresh token sudah 384-bit random dari CSPRNG (bukan
// password pilihan manusia yang lemah/bisa ditebak), jadi tidak butuh
// hash "mahal" buat tahan brute-force, cukup satu-arah biar nilai
// mentahnya tidak nyangkut plaintext di tabel kalau database dicuri.
export function hashRefreshToken(token: string): string {
  return crypto.createHash("sha256").update(token).digest("hex");
}
