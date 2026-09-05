// Terima request, panggil service, bentuk response HTTP — TIDAK ADA
// logic bisnis di sini (lihat aturan konvensi DESIGN.md bagian 3).

import { asyncHandler } from "../../utils/asyncHandler.js";
import * as groupsService from "./groups.service.js";

export const listGroups = asyncHandler(async (req, res) => {
  const { vaultId } = req.params as { vaultId: string };
  res.json(await groupsService.listGroups(vaultId));
});

export const getGroup = asyncHandler(async (req, res) => {
  const { vaultId, id } = req.params as { vaultId: string; id: string };
  res.json(await groupsService.getGroup(vaultId, id));
});

export const createGroup = asyncHandler(async (req, res) => {
  const { vaultId } = req.params as { vaultId: string };
  const result = await groupsService.createGroup(vaultId, req.body);
  res.status(201).json(result);
});

export const updateGroup = asyncHandler(async (req, res) => {
  const { vaultId, id } = req.params as { vaultId: string; id: string };
  const result = await groupsService.updateGroup(vaultId, id, req.body);
  res.json(result);
});

export const deleteGroup = asyncHandler(async (req, res) => {
  const { vaultId, id } = req.params as { vaultId: string; id: string };
  await groupsService.deleteGroup(vaultId, id);
  res.status(204).send();
});
