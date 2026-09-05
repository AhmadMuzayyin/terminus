# Multi-stage: stage "build" compile TypeScript + generate Prisma
# client, stage runtime cuma bawa hasil build + node_modules production
# saja (image akhir lebih kecil, tidak ikut bawa devDependencies).

FROM node:20-slim AS build
WORKDIR /app
COPY package.json package-lock.json* ./
RUN npm install
COPY prisma ./prisma
RUN npx prisma generate
COPY tsconfig.json tsconfig.build.json ./
COPY src ./src
RUN npm run build

FROM node:20-slim AS runtime
WORKDIR /app
ENV NODE_ENV=production
COPY package.json package-lock.json* ./
RUN npm install --omit=dev
COPY --from=build /app/node_modules/.prisma ./node_modules/.prisma
COPY --from=build /app/dist ./dist
COPY prisma ./prisma

EXPOSE 4000
# Migrasi dijalankan SEKALI waktu container start (aman diulang —
# `prisma migrate deploy` idempotent, skip migrasi yang sudah
# ter-apply) baru start server-nya.
CMD ["sh", "-c", "npx prisma migrate deploy && node dist/index.js"]
