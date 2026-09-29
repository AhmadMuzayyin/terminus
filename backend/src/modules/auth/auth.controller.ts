// Terima request, panggil service, bentuk response HTTP — TIDAK ADA
// logic bisnis di sini (lihat aturan konvensi DESIGN.md bagian 3).

import { asyncHandler } from "../../utils/asyncHandler.js";
import * as authService from "./auth.service.js";

export const register = asyncHandler(async (req, res) => {
  const { email, password, fullName } = req.body as { email: string; password: string; fullName: string };
  const result = await authService.register(email, password, fullName);
  res.status(201).json(result);
});

export const login = asyncHandler(async (req, res) => {
  const { email, password } = req.body as { email: string; password: string };
  const result = await authService.login(email, password);
  res.json(result);
});

export const refresh = asyncHandler(async (req, res) => {
  const { refreshToken } = req.body as { refreshToken: string };
  const result = await authService.refresh(refreshToken);
  res.json(result);
});

export const logout = asyncHandler(async (req, res) => {
  const { refreshToken } = req.body as { refreshToken: string };
  await authService.logout(refreshToken);
  res.status(204).send();
});

// req.user dijamin ADA di sini — route-nya dipasang di belakang
// `requireAuth` (lihat auth.routes.ts), yang menolak (401) request
// tanpa itu SEBELUM sampai ke handler ini.
export const me = asyncHandler(async (req, res) => {
  const result = await authService.me(req.user!.id);
  res.json(result);
});

export const updateMe = asyncHandler(async (req, res) => {
  const changes = req.body as { fullName?: string; email?: string; currentPassword?: string };
  const result = await authService.updateMe(req.user!.id, changes);
  res.json(result);
});

export const changePassword = asyncHandler(async (req, res) => {
  const { currentPassword, newPassword } = req.body as { currentPassword: string; newPassword: string };
  const result = await authService.changePassword(req.user!.id, currentPassword, newPassword);
  res.json(result);
});
