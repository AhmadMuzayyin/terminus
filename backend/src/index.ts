// Entrypoint proses: load config, bikin app, listen di port. Dijaga
// SETIPIS mungkin (cuma "nyalain" — logic app-nya sendiri di app.ts)
// supaya `app.ts` bisa dites tanpa nge-listen port beneran.

import { createApp } from "./app.js";
import { config } from "./config/env.js";
import { prisma } from "./db/client.js";

const app = createApp();

const server = app.listen(config.PORT, () => {
  // eslint-disable-next-line no-console -- log startup memang tujuannya tampil di console/journal
  console.log(`terminus-server listening on port ${config.PORT}`);
});

// Matikan koneksi DB rapi waktu proses di-stop (Ctrl+C lokal, atau
// `docker stop` yang kirim SIGTERM) — biar tidak nyangkut koneksi
// setengah jalan.
async function shutdown(signal: string) {
  // eslint-disable-next-line no-console -- log shutdown, sama alasan dengan log startup di atas
  console.log(`Menerima ${signal}, mematikan server...`);
  server.close();
  await prisma.$disconnect();
  process.exit(0);
}

process.on("SIGINT", () => void shutdown("SIGINT"));
process.on("SIGTERM", () => void shutdown("SIGTERM"));
