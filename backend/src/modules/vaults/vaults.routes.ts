import { Router } from "express";

import { requireAuth } from "../../middleware/auth.js";
import { requireVaultMember, requireVaultOwner } from "../../middleware/vaultAccess.js";
import { validate } from "../../middleware/validate.js";
import { addMember, createVault, listVaults, removeMember } from "./vaults.controller.js";
import { addMemberSchema, createVaultSchema, removeMemberSchema } from "./vaults.schema.js";

export const vaultsRouter = Router();

// SEMUA route di modul ini butuh login — dipasang sekali di sini
// (bukan per-route) berlaku buat seluruh router.
vaultsRouter.use(requireAuth);

vaultsRouter.post("/", validate(createVaultSchema), createVault);
vaultsRouter.get("/", listVaults);
// `validate` jalan DULUAN (mastiin :vaultId formatnya UUID valid
// sebelum dipakai query DB), BARU requireVaultMember (cek keanggotaan)
// lalu requireVaultOwner (cek role) — urutan ini penting, masing-masing
// middleware asumsi yang sebelumnya sudah lolos.
vaultsRouter.post(
  "/:vaultId/members",
  validate(addMemberSchema),
  requireVaultMember,
  requireVaultOwner,
  addMember,
);
vaultsRouter.delete(
  "/:vaultId/members/:userId",
  validate(removeMemberSchema),
  requireVaultMember,
  requireVaultOwner,
  removeMember,
);
