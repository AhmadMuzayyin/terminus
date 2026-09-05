import { z } from "zod";

// String biasa (bukan enum ketat) — SAMA alasan dengan
// `terminus_core::export::json::BackupHost.kind`: skema stabil biarpun
// representasi enum Rust `ConnectionKind` berubah nanti.
const kindSchema = z.enum(["ssh", "cisco_ios"]);

export const listHostsSchema = z.object({
  params: z.object({ vaultId: z.string().uuid("vaultId tidak valid") }),
});

export const createHostSchema = z.object({
  params: z.object({ vaultId: z.string().uuid("vaultId tidak valid") }),
  body: z.object({
    label: z.string().min(1, "Label wajib diisi"),
    host: z.string().min(1, "Host/IP wajib diisi"),
    port: z.coerce.number().int().min(1).max(65535).default(22),
    username: z.string().min(1, "Username wajib diisi"),
    kind: kindSchema.default("ssh"),
    groupId: z.string().uuid("groupId tidak valid").nullable().optional(),
    tags: z.array(z.string()).default([]),
    terminalTheme: z.string().nullable().optional(),
  }),
});

export const hostIdParamSchema = z.object({
  params: z.object({
    vaultId: z.string().uuid("vaultId tidak valid"),
    id: z.string().uuid("id tidak valid"),
  }),
});

export const updateHostSchema = z.object({
  params: z.object({
    vaultId: z.string().uuid("vaultId tidak valid"),
    id: z.string().uuid("id tidak valid"),
  }),
  body: z.object({
    label: z.string().min(1).optional(),
    host: z.string().min(1).optional(),
    port: z.coerce.number().int().min(1).max(65535).optional(),
    username: z.string().min(1).optional(),
    kind: kindSchema.optional(),
    groupId: z.string().uuid("groupId tidak valid").nullable().optional(),
    tags: z.array(z.string()).optional(),
    terminalTheme: z.string().nullable().optional(),
  }),
});

export const setHostSecretSchema = z.object({
  params: z.object({
    vaultId: z.string().uuid("vaultId tidak valid"),
    id: z.string().uuid("id tidak valid"),
  }),
  body: z.object({
    password: z.string().min(1, "Password wajib diisi"),
  }),
});
