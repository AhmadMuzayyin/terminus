import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    // Semua suite di sini jalan lawan SATU database MySQL sungguhan
    // yang sama (bukan DB terisolasi per-test/per-file) — kalau file
    // test dijalankan PARALEL (default vitest), satu suite bisa
    // menghapus/menimpa data yang lagi dipakai suite lain di saat yang
    // sama (mis. auth.test.ts mengosongkan tabel users tepat waktu
    // vaults.test.ts lagi butuh user itu). Dipaksa SEKUENSIAL di sini
    // supaya tiap file test yang wipe/seed tabelnya sendiri tidak
    // saling tabrakan.
    fileParallelism: false,
  },
});
