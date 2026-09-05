// Singleton PrismaClient — SEMUA modul akses tabel lewat import
// `prisma` dari sini, jangan `new PrismaClient()` di tempat lain
// (bikin banyak connection pool kalau diulang-ulang).

import { PrismaClient } from "@prisma/client";

export const prisma = new PrismaClient();
