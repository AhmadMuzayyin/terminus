// Bukti nyata (bukan cuma "compile") endpoint /health beneran jalan
// DAN beneran nyambung ke MySQL sungguhan (bukan mock) — mirip filosofi
// test desktop app (`crates/vault`) yang selalu pakai DB beneran, bukan
// in-memory fake.

import { describe, expect, it } from "vitest";
import request from "supertest";

import { createApp } from "../src/app.js";

describe("GET /health", () => {
  it("balikin status ok dan db connected kalau MySQL bisa dijangkau", async () => {
    const app = createApp();
    const response = await request(app).get("/health");

    expect(response.status).toBe(200);
    expect(response.body).toEqual({ status: "ok", db: "connected" });
  });
});
