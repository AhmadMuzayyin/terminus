// Terima request, panggil service, bentuk response HTTP — TIDAK ADA
// logic bisnis di sini (lihat aturan konvensi DESIGN.md bagian 3).

import { asyncHandler } from "../../utils/asyncHandler.js";
import * as hostsService from "./hosts.service.js";

export const listHosts = asyncHandler(async (req, res) => {
  const { vaultId } = req.params as { vaultId: string };
  res.json(await hostsService.listHosts(vaultId));
});

export const getHost = asyncHandler(async (req, res) => {
  const { vaultId, id } = req.params as { vaultId: string; id: string };
  res.json(await hostsService.getHost(vaultId, id));
});

export const createHost = asyncHandler(async (req, res) => {
  const { vaultId } = req.params as { vaultId: string };
  const result = await hostsService.createHost(vaultId, req.body);
  res.status(201).json(result);
});

export const updateHost = asyncHandler(async (req, res) => {
  const { vaultId, id } = req.params as { vaultId: string; id: string };
  const result = await hostsService.updateHost(vaultId, id, req.body);
  res.json(result);
});

export const deleteHost = asyncHandler(async (req, res) => {
  const { vaultId, id } = req.params as { vaultId: string; id: string };
  await hostsService.deleteHost(vaultId, id);
  res.status(204).send();
});

// PUT (bukan PATCH) — sengaja replace SELURUH secret host itu,
// tidak ada konsep "tambah password" parsial.
export const setHostSecret = asyncHandler(async (req, res) => {
  const { vaultId, id } = req.params as { vaultId: string; id: string };
  const { password } = req.body as { password: string };
  await hostsService.setSecret(vaultId, id, password);
  res.status(204).send();
});

// Dipanggil client CUMA waktu benar-benar mau connect SSH (lihat
// DESIGN.md bagian 5.5) — TIDAK PERNAH dipanggil buat render daftar
// host biasa (`listHosts`/`getHost` cuma expose `hasPassword: boolean`).
export const getHostSecret = asyncHandler(async (req, res) => {
  const { vaultId, id } = req.params as { vaultId: string; id: string };
  const password = await hostsService.getSecret(vaultId, id);
  res.json({ password });
});
