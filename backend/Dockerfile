# Multi-stage: stage "build" compile TypeScript + generate Prisma
# client, stage runtime cuma bawa hasil build + node_modules production
# saja (image akhir lebih kecil, tidak ikut bawa devDependencies).

FROM node:24-slim AS build
# Prisma butuh libssl untuk mendeteksi & memuat engine yang benar —
# tanpa ini engine salah target dan CLI mencoba mengunduh saat runtime.
RUN apt-get update && apt-get install -y --no-install-recommends openssl \
  && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci
COPY prisma ./prisma
RUN npx prisma generate
COPY tsconfig.json tsconfig.build.json ./
COPY src ./src
RUN npm run build

FROM node:24-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends openssl \
  && rm -rf /var/lib/apt/lists/*
WORKDIR /app
ENV NODE_ENV=production
COPY package.json package-lock.json ./
RUN npm ci --omit=dev && npm cache clean --force
COPY --from=build /app/node_modules/.prisma ./node_modules/.prisma
COPY --from=build /app/dist ./dist
COPY prisma ./prisma

# File aplikasi tetap milik root (tidak bisa diubah proses server);
# proses jalan sebagai user `node` bawaan image, bukan root.
USER node

EXPOSE 4000
HEALTHCHECK --interval=30s --timeout=5s --start-period=30s --retries=3 \
  CMD ["node", "-e", "fetch('http://127.0.0.1:' + (process.env.PORT || 4000) + '/health').then(r => process.exit(r.ok ? 0 : 1)).catch(() => process.exit(1))"]

# Migrasi dijalankan SEKALI waktu container start (aman diulang —
# `prisma migrate deploy` idempotent, skip migrasi yang sudah
# ter-apply) baru start server-nya. `--no`: pakai CLI prisma yang
# terpasang dari lockfile, jangan pernah unduh versi lain saat runtime.
CMD ["sh", "-c", "npx --no prisma migrate deploy && node dist/index.js"]
