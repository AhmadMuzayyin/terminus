import { z } from "zod";

export const listGroupsSchema = z.object({
  params: z.object({ vaultId: z.string().uuid("vaultId tidak valid") }),
});

export const createGroupSchema = z.object({
  params: z.object({ vaultId: z.string().uuid("vaultId tidak valid") }),
  body: z.object({
    // Lihat komentar sama di hosts.schema.ts `createHostSchema.id`.
    id: z.string().uuid("id tidak valid").optional(),
    name: z.string().min(1, "Nama grup wajib diisi"),
    subtitle: z.string().nullable().optional(),
    parentId: z.string().uuid("parentId tidak valid").nullable().optional(),
  }),
});

export const groupIdParamSchema = z.object({
  params: z.object({
    vaultId: z.string().uuid("vaultId tidak valid"),
    id: z.string().uuid("id tidak valid"),
  }),
});

export const updateGroupSchema = z.object({
  params: z.object({
    vaultId: z.string().uuid("vaultId tidak valid"),
    id: z.string().uuid("id tidak valid"),
  }),
  body: z.object({
    name: z.string().min(1).optional(),
    subtitle: z.string().nullable().optional(),
    parentId: z.string().uuid("parentId tidak valid").nullable().optional(),
  }),
});
