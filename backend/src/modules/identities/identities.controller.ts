// Terima request, panggil service, bentuk response HTTP — TIDAK ADA
// logic bisnis di sini (lihat aturan konvensi DESIGN.md bagian 3).

import { asyncHandler } from "../../utils/asyncHandler.js";
import * as identitiesService from "./identities.service.js";

export const listIdentities = asyncHandler(async (req, res) => {
  const { vaultId } = req.params as { vaultId: string };
  res.json(await identitiesService.listIdentities(vaultId));
});

export const getIdentity = asyncHandler(async (req, res) => {
  const { vaultId, id } = req.params as { vaultId: string; id: string };
  res.json(await identitiesService.getIdentity(vaultId, id));
});

export const createIdentity = asyncHandler(async (req, res) => {
  const { vaultId } = req.params as { vaultId: string };
  const result = await identitiesService.createIdentity(vaultId, req.body);
  res.status(201).json(result);
});

export const updateIdentity = asyncHandler(async (req, res) => {
  const { vaultId, id } = req.params as { vaultId: string; id: string };
  const result = await identitiesService.updateIdentity(vaultId, id, req.body);
  res.json(result);
});

export const deleteIdentity = asyncHandler(async (req, res) => {
  const { vaultId, id } = req.params as { vaultId: string; id: string };
  await identitiesService.deleteIdentity(vaultId, id);
  res.status(204).send();
});

// Dipakai client waktu "pick identity" ngisi form host baru (mirror
// `on_identity_picked` di desktop app) — bukan buat render list.
export const getIdentitySecret = asyncHandler(async (req, res) => {
  const { vaultId, id } = req.params as { vaultId: string; id: string };
  const password = await identitiesService.getIdentitySecret(vaultId, id);
  res.json({ password });
});
