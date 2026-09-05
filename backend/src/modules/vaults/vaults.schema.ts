import { z } from "zod";

export const createVaultSchema = z.object({
  body: z.object({
    name: z.string().min(1, "Nama vault wajib diisi"),
  }),
});

export const vaultIdParamSchema = z.object({
  params: z.object({
    vaultId: z.string().uuid("vaultId tidak valid"),
  }),
});

export const addMemberSchema = z.object({
  params: z.object({
    vaultId: z.string().uuid("vaultId tidak valid"),
  }),
  body: z.object({
    email: z.string().email("Email tidak valid"),
  }),
});

export const removeMemberSchema = z.object({
  params: z.object({
    vaultId: z.string().uuid("vaultId tidak valid"),
    userId: z.string().uuid("userId tidak valid"),
  }),
});
