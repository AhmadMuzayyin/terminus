import { Router } from "express";

import { requireAuth } from "../../middleware/auth.js";
import { validate } from "../../middleware/validate.js";
import { changePassword, login, logout, me, refresh, register, updateMe } from "./auth.controller.js";
import {
  changePasswordSchema,
  loginSchema,
  logoutSchema,
  refreshSchema,
  registerSchema,
  updateMeSchema,
} from "./auth.schema.js";

export const authRouter = Router();

authRouter.post("/register", validate(registerSchema), register);
authRouter.post("/login", validate(loginSchema), login);
authRouter.post("/refresh", validate(refreshSchema), refresh);
authRouter.post("/logout", validate(logoutSchema), logout);
// "Siapa saya" — cara termudah verifikasi access token beneran valid,
// dipakai juga sebagai bukti `requireAuth` bekerja di test.
authRouter.get("/me", requireAuth, me);
// Profil akun (DESIGN.md Milestone 6).
authRouter.patch("/me", requireAuth, validate(updateMeSchema), updateMe);
authRouter.put("/me/password", requireAuth, validate(changePasswordSchema), changePassword);
