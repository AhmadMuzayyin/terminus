// Terima request, panggil service, bentuk response HTTP — TIDAK ADA
// logic bisnis di sini (lihat aturan konvensi DESIGN.md bagian 3).

import { asyncHandler } from "../../utils/asyncHandler.js";
import * as vaultsService from "./vaults.service.js";

export const createVault = asyncHandler(async (req, res) => {
  const { name } = req.body as { name: string };
  const result = await vaultsService.createVault(req.user!.id, name);
  res.status(201).json(result);
});

export const listVaults = asyncHandler(async (req, res) => {
  const result = await vaultsService.listVaultsForUser(req.user!.id);
  res.json(result);
});

// Dipasang di belakang requireVaultMember + requireVaultOwner (lihat
// vaults.routes.ts) — sampai sini requester DIJAMIN owner vault ini.
export const addMember = asyncHandler(async (req, res) => {
  const { vaultId } = req.params as { vaultId: string };
  const { email } = req.body as { email: string };
  const result = await vaultsService.addMember(vaultId, email);
  res.status(201).json(result);
});

export const removeMember = asyncHandler(async (req, res) => {
  const { vaultId, userId } = req.params as { vaultId: string; userId: string };
  await vaultsService.removeMember(vaultId, userId);
  res.status(204).send();
});
