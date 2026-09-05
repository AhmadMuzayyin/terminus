import { Router } from "express";

import { requireAuth } from "../../middleware/auth.js";
import { validate } from "../../middleware/validate.js";
import { login, logout, me, refresh, register } from "./auth.controller.js";
import { loginSchema, logoutSchema, refreshSchema, registerSchema } from "./auth.schema.js";

export const authRouter = Router();

authRouter.post("/register", validate(registerSchema), register);
authRouter.post("/login", validate(loginSchema), login);
authRouter.post("/refresh", validate(refreshSchema), refresh);
authRouter.post("/logout", validate(logoutSchema), logout);
// "Siapa saya" — cara termudah verifikasi access token beneran valid,
// dipakai juga sebagai bukti `requireAuth` bekerja di test.
authRouter.get("/me", requireAuth, me);
