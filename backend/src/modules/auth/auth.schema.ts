import { z } from "zod";

export const registerSchema = z.object({
  body: z.object({
    email: z.string().email("Email tidak valid"),
    password: z.string().min(8, "Password minimal 8 karakter"),
  }),
});

// Sengaja skema TERPISAH dari registerSchema meski bentuknya sama
// sekarang — login & register punya aturan validasi yang bisa
// menyimpang ke depannya (mis. login tidak perlu aturan panjang
// minimum password), jangan digabung cuma karena kebetulan identik.
export const loginSchema = z.object({
  body: z.object({
    email: z.string().email("Email tidak valid"),
    password: z.string().min(1, "Password wajib diisi"),
  }),
});

export const refreshSchema = z.object({
  body: z.object({
    refreshToken: z.string().min(1, "refreshToken wajib diisi"),
  }),
});

export const logoutSchema = refreshSchema;
