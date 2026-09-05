// Bikin Express app + pasang middleware global. Dipisah dari
// `index.ts` (yang urusannya cuma "listen di port") supaya app-nya
// bisa di-import LANGSUNG di test (`supertest(app)`) tanpa perlu
// benar-benar bind ke port jaringan.

import cors from "cors";
import express, { type Express } from "express";

import { prisma } from "./db/client.js";
import { errorHandler } from "./middleware/errorHandler.js";
import { requireAuth } from "./middleware/auth.js";
import { requireVaultMember, validateVaultIdParam } from "./middleware/vaultAccess.js";
import { authRouter } from "./modules/auth/auth.routes.js";
import { groupsRouter } from "./modules/groups/groups.routes.js";
import { hostsRouter } from "./modules/hosts/hosts.routes.js";
import { identitiesRouter } from "./modules/identities/identities.routes.js";
import { vaultsRouter } from "./modules/vaults/vaults.routes.js";

export function createApp(): Express {
  const app = express();

  app.use(cors());
  app.use(express.json());

  // Health check — cek proses HIDUP (200 tanpa syarat) DAN koneksi DB
  // beneran jalan (query `SELECT 1`). Dipakai Docker healthcheck /
  // load balancer, juga cara tercepat mastiin `DATABASE_URL` benar
  // waktu setup awal self-host.
  app.get("/health", async (_req, res) => {
    try {
      await prisma.$queryRaw`SELECT 1`;
      res.json({ status: "ok", db: "connected" });
    } catch (error) {
      res.status(503).json({ status: "error", db: "unreachable", message: (error as Error).message });
    }
  });

  app.use("/api/v1/auth", authRouter);
  app.use("/api/v1/vaults", vaultsRouter);
  // Hosts/groups/identities SEMUA butuh login DAN keanggotaan vault
  // yang ditunjuk `:vaultId` di path-nya — dipasang sekali di sini
  // (mount-level), bukan diulang di tiap route dalam masing-masing
  // router (yang sudah `mergeParams: true` buat bisa baca `:vaultId`
  // parent ini).
  app.use(
    "/api/v1/vaults/:vaultId/hosts",
    requireAuth,
    validateVaultIdParam,
    requireVaultMember,
    hostsRouter,
  );
  app.use(
    "/api/v1/vaults/:vaultId/groups",
    requireAuth,
    validateVaultIdParam,
    requireVaultMember,
    groupsRouter,
  );
  app.use(
    "/api/v1/vaults/:vaultId/identities",
    requireAuth,
    validateVaultIdParam,
    requireVaultMember,
    identitiesRouter,
  );

  // WAJIB PALING TERAKHIR — Express nentuin ini "error handler" cuma
  // dari arity 4 parameter (err, req, res, next), bukan dari nama atau
  // urutan import, tapi TETAP harus didaftarkan setelah semua route
  // supaya `next(err)` dari middleware/controller manapun di atas bisa
  // nyampe ke sini.
  app.use(errorHandler);

  return app;
}
