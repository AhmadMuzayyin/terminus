// Enkripsi/dekripsi password host & identity, pakai ChaCha20-Poly1305
// bawaan `node:crypto` — SAMA PRIMITIVE dengan
// `terminus_vault::crypto::encrypt/decrypt` di desktop app, TAPI
// key-nya BEDA SUMBER: di sini SATU kunci milik SERVER
// (`SERVER_MASTER_KEY`), bukan diturunkan dari master password
// per-user (lihat DESIGN.md bagian 2, "server-side encryption at
// rest"). Layout blob output SENGAJA SAMA dengan Rust: `nonce (12
// byte) || ciphertext || auth tag (16 byte)` — nonce & tag TIDAK
// rahasia, cuma harus unik per enkripsi, jadi aman disimpan bareng
// ciphertext dalam satu kolom `secrets.encrypted_data`.

import crypto from "node:crypto";

import { config } from "../config/env.js";

const ALGORITHM = "chacha20-poly1305";
const NONCE_LENGTH = 12;
const AUTH_TAG_LENGTH = 16;

function loadKey(): Buffer {
  return Buffer.from(config.SERVER_MASTER_KEY, "base64");
}

export function encryptSecret(plaintext: string): Buffer {
  const nonce = crypto.randomBytes(NONCE_LENGTH);
  const cipher = crypto.createCipheriv(ALGORITHM, loadKey(), nonce, { authTagLength: AUTH_TAG_LENGTH });
  const ciphertext = Buffer.concat([cipher.update(plaintext, "utf8"), cipher.final()]);
  const authTag = cipher.getAuthTag();
  return Buffer.concat([nonce, ciphertext, authTag]);
}

export function decryptSecret(blob: Buffer): string {
  if (blob.length < NONCE_LENGTH + AUTH_TAG_LENGTH) {
    throw new Error("Blob secret terlalu pendek — data korup atau bukan hasil encryptSecret");
  }
  const nonce = blob.subarray(0, NONCE_LENGTH);
  const authTag = blob.subarray(blob.length - AUTH_TAG_LENGTH);
  const ciphertext = blob.subarray(NONCE_LENGTH, blob.length - AUTH_TAG_LENGTH);

  const decipher = crypto.createDecipheriv(ALGORITHM, loadKey(), nonce, { authTagLength: AUTH_TAG_LENGTH });
  decipher.setAuthTag(authTag);
  return Buffer.concat([decipher.update(ciphertext), decipher.final()]).toString("utf8");
}
