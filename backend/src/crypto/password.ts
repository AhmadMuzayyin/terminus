// Hash/verify password LOGIN pakai Argon2id — SAMA ALGORITMA dengan
// `terminus-vault::crypto::derive_key` di desktop app (konsistensi
// postur keamanan lintas project, lihat DESIGN.md bagian 2). Beda
// tujuan dari sana: di sini murni buat VERIFIKASI login (bandingkan
// hash), bukan buat menurunkan encryption key.

import argon2 from "argon2";

export function hashPassword(password: string): Promise<string> {
  return argon2.hash(password);
}

export function verifyPassword(hash: string, password: string): Promise<boolean> {
  return argon2.verify(hash, password);
}
