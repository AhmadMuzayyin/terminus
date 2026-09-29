import { z } from "zod";

// Dipakai register & PATCH /me — satu aturan nama (trim dulu, supaya
// "   " tidak lolos sebagai nama).
const fullNameField = z
  .string()
  .trim()
  .min(1, "Nama lengkap wajib diisi")
  .max(100, "Nama lengkap maksimal 100 karakter");

const newPasswordField = z.string().min(8, "Password minimal 8 karakter");

export const registerSchema = z.object({
  body: z.object({
    email: z.string().email("Email tidak valid"),
    password: newPasswordField,
    fullName: fullNameField,
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

// Ganti email WAJIB `currentPassword` — dicek di service (butuh tahu
// email lama), bukan di sini. Minimal satu field yang diubah.
export const updateMeSchema = z.object({
  body: z
    .object({
      fullName: fullNameField.optional(),
      email: z.string().email("Email tidak valid").optional(),
      currentPassword: z.string().min(1).optional(),
    })
    .refine((b) => b.fullName !== undefined || b.email !== undefined, {
      message: "Isi minimal salah satu: fullName atau email",
    }),
});

export const changePasswordSchema = z.object({
  body: z.object({
    currentPassword: z.string().min(1, "Password saat ini wajib diisi"),
    newPassword: newPasswordField,
  }),
});
