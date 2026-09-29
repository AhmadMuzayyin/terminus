// Logic bisnis murni — TIDAK IMPORT apa pun dari Express (lihat aturan
// konvensi DESIGN.md bagian 3), supaya gampang dites tanpa mock HTTP.

import { config } from "../../config/env.js";
import { hashPassword, verifyPassword } from "../../crypto/password.js";
import { generateRefreshToken, hashRefreshToken, signAccessToken } from "../../crypto/tokens.js";
import { prisma } from "../../db/client.js";
import { ConflictError, ForbiddenError, UnauthorizedError } from "../../errors.js";

interface TokenPair {
  accessToken: string;
  refreshToken: string;
}

// `fullName` null = akun lama yang dibuat sebelum kolom ini ada (DESIGN.md
// Milestone 6).
interface PublicUser {
  id: string;
  email: string;
  fullName: string | null;
}

interface AuthResult extends TokenPair {
  user: PublicUser;
}

function toPublicUser(user: { id: string; email: string; fullName: string | null }): PublicUser {
  return { id: user.id, email: user.email, fullName: user.fullName };
}

// Password SALAH di endpoint profil -> 403, SENGAJA bukan 401: client
// desktop menganggap 401 = access token kedaluwarsa lalu refresh+retry
// otomatis (DESIGN.md Milestone 6).
const wrongCurrentPassword = () => new ForbiddenError("Password saat ini salah");

async function issueTokens(userId: string): Promise<TokenPair> {
  const accessToken = signAccessToken(userId);
  const refreshToken = generateRefreshToken();
  const tokenHash = hashRefreshToken(refreshToken);
  const expiresAt = new Date(Date.now() + config.REFRESH_TOKEN_TTL_SECONDS * 1000);

  await prisma.refreshToken.create({ data: { userId, tokenHash, expiresAt } });

  return { accessToken, refreshToken };
}

// Register CUMA jalan waktu tabel users masih kosong (first-run
// bootstrap admin) — lihat DESIGN.md bagian 5.2. User berikutnya masuk
// lewat invite ke vault (modul vaults, belum dikerjakan), BUKAN
// endpoint register ini — server self-hosted ini bukan layanan publik.
export async function register(email: string, password: string, fullName: string): Promise<AuthResult> {
  const existingUserCount = await prisma.user.count();
  if (existingUserCount > 0) {
    throw new ForbiddenError(
      "Registrasi publik tertutup — server ini sudah punya user. Minta admin invite Anda ke vault.",
    );
  }

  const passwordHash = await hashPassword(password);
  const user = await prisma.user.create({ data: { email, passwordHash, fullName } });
  const tokens = await issueTokens(user.id);

  return { user: toPublicUser(user), ...tokens };
}

export async function login(email: string, password: string): Promise<AuthResult> {
  const user = await prisma.user.findUnique({ where: { email } });

  // Pesan error SENGAJA generic (sama persis) buat "user tidak ada"
  // MAUPUN "password salah" — jangan bocorkan mana yang salah, standar
  // praktik biar tidak bisa dipakai enumerasi email yang terdaftar.
  const invalidCredentials = new UnauthorizedError("Email atau password salah");
  if (!user) {
    throw invalidCredentials;
  }
  const validPassword = await verifyPassword(user.passwordHash, password);
  if (!validPassword) {
    throw invalidCredentials;
  }

  const tokens = await issueTokens(user.id);
  return { user: toPublicUser(user), ...tokens };
}

export async function refresh(refreshToken: string): Promise<TokenPair> {
  const tokenHash = hashRefreshToken(refreshToken);
  const stored = await prisma.refreshToken.findUnique({ where: { tokenHash } });

  const invalid = new UnauthorizedError("Refresh token tidak valid, sudah dicabut, atau kedaluwarsa");
  if (!stored || stored.revokedAt !== null || stored.expiresAt.getTime() < Date.now()) {
    throw invalid;
  }

  // Rotate: cabut token lama SEBELUM keluarkan yang baru — kalau
  // refresh token yang sama dipakai lagi setelah ini (mis. dicuri lalu
  // dipakai penyerang SETELAH pemilik asli sudah refresh duluan),
  // percobaan kedua otomatis ditolak karena yang lama sudah revoked.
  await prisma.refreshToken.update({
    where: { tokenHash },
    data: { revokedAt: new Date() },
  });

  return issueTokens(stored.userId);
}

export async function logout(refreshToken: string): Promise<void> {
  const tokenHash = hashRefreshToken(refreshToken);
  // `updateMany` (bukan `update`) — logout tetap "berhasil" (204) meski
  // token yang dikirim sudah tidak ada/sudah revoked duluan, caller
  // tidak perlu handle error di alur yang seharusnya selalu aman
  // dipanggil kapan pun.
  await prisma.refreshToken.updateMany({
    where: { tokenHash, revokedAt: null },
    data: { revokedAt: new Date() },
  });
}

async function findExistingUser(userId: string) {
  const user = await prisma.user.findUnique({ where: { id: userId } });
  if (!user) {
    // Access token valid tapi user-nya sudah tidak ada (mis. dihapus
    // admin sebelum token expired) — kasus langka, tetap harus ditolak
    // eksplisit bukan crash.
    throw new UnauthorizedError("User pemilik token ini sudah tidak ada");
  }
  return user;
}

export async function me(userId: string): Promise<PublicUser> {
  return toPublicUser(await findExistingUser(userId));
}

// Ganti nama bebas; ganti email WAJIB password saat ini benar (email =
// identitas login). Email sama dengan yang sekarang = bukan perubahan,
// tidak butuh password.
export async function updateMe(
  userId: string,
  changes: { fullName?: string; email?: string; currentPassword?: string },
): Promise<PublicUser> {
  const user = await findExistingUser(userId);
  const emailChanged = changes.email !== undefined && changes.email !== user.email;

  if (emailChanged) {
    if (!changes.currentPassword || !(await verifyPassword(user.passwordHash, changes.currentPassword))) {
      throw wrongCurrentPassword();
    }
    const taken = await prisma.user.findUnique({ where: { email: changes.email } });
    if (taken) {
      throw new ConflictError("Email sudah dipakai akun lain");
    }
  }

  const updated = await prisma.user.update({
    where: { id: userId },
    data: {
      ...(changes.fullName !== undefined ? { fullName: changes.fullName } : {}),
      ...(emailChanged ? { email: changes.email } : {}),
    },
  });
  return toPublicUser(updated);
}

// Cabut SEMUA refresh token user ini (semua perangkat — desktop lain,
// mobile app nanti, dst), lalu terbitkan pasangan BARU supaya perangkat
// yang mengganti password tetap login. Access token lain yang masih
// berlaku (<= ACCESS_TOKEN_TTL_SECONDS) tetap valid sampai habis — batasan
// JWT stateless, lihat DESIGN.md Milestone 6.
export async function changePassword(
  userId: string,
  currentPassword: string,
  newPassword: string,
): Promise<TokenPair> {
  const user = await findExistingUser(userId);
  if (!(await verifyPassword(user.passwordHash, currentPassword))) {
    throw wrongCurrentPassword();
  }

  const passwordHash = await hashPassword(newPassword);
  await prisma.$transaction([
    prisma.user.update({ where: { id: userId }, data: { passwordHash } }),
    prisma.refreshToken.updateMany({ where: { userId, revokedAt: null }, data: { revokedAt: new Date() } }),
  ]);
  return issueTokens(userId);
}
