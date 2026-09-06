import { z } from "zod";

export const listIdentitiesSchema = z.object({
  params: z.object({ vaultId: z.string().uuid("vaultId tidak valid") }),
});

// Password WAJIB waktu create — beda dari host, Identity tanpa
// password tidak ada gunanya sama sekali (lihat komentar
// identities.service.ts).
export const createIdentitySchema = z.object({
  params: z.object({ vaultId: z.string().uuid("vaultId tidak valid") }),
  body: z.object({
    // Lihat komentar sama di hosts.schema.ts `createHostSchema.id`.
    id: z.string().uuid("id tidak valid").optional(),
    label: z.string().min(1, "Label wajib diisi"),
    username: z.string().min(1, "Username wajib diisi"),
    password: z.string().min(1, "Password wajib diisi"),
  }),
});

export const identityIdParamSchema = z.object({
  params: z.object({
    vaultId: z.string().uuid("vaultId tidak valid"),
    id: z.string().uuid("id tidak valid"),
  }),
});

export const updateIdentitySchema = z.object({
  params: z.object({
    vaultId: z.string().uuid("vaultId tidak valid"),
    id: z.string().uuid("id tidak valid"),
  }),
  body: z.object({
    label: z.string().min(1).optional(),
    username: z.string().min(1).optional(),
    // Kosongkan/hilangkan field ini = password TIDAK diubah — sama
    // filosofi dengan field Password di panel host desktop app.
    password: z.string().min(1).optional(),
  }),
});
