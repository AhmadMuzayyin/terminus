//! Konversi murni `terminus_term_emulator::TerminalGrid` (grid sel
//! platform-agnostic) ke `TermRow`/`TermRun` (tipe hasil codegen Slint
//! dari `ui/models.slint`) yang beneran dirender `TerminalView` di
//! `ui/pages/page-console.slint`.
//!
//! Sengaja dipisah dari `state.rs` (yang isinya wiring async/vault) —
//! modul ini TIDAK butuh `AppState`, murni fungsi `&TerminalGrid ->
//! Vec<TermRow>`, jadi gampang dites sendiri tanpa `AppWindow`/tokio.
//!
//! Kursor DIBAKAR langsung ke warna sel (invert fg/bg di posisi kursor)
//! di sini, bukan di-overlay terpisah di Slint — menghindari perlu
//! hitung posisi piksel kursor manual (lebar karakter monospace,
//! offset scroll, dst) di sisi UI.

use crate::{TermRow, TermRun};
use terminus_term_emulator::grid::Rgb;
use terminus_term_emulator::TerminalGrid;
use slint::{ModelRc, VecModel};

/// Ukuran PTY tetap buat v1 — belum ada reflow dinamis mengikuti ukuran
/// jendela (lihat catatan di README bagian "Batasan v1 terminal").
pub const TERM_COLS: u16 = 100;
pub const TERM_ROWS: u16 = 32;

/// Satu "run" dalam representasi ANTARA yang murni data (`Send`), belum
/// jadi tipe Slint (`TermRun` membungkus `slint::Color`, tapi `TermRow`
/// membungkus `ModelRc<TermRun>` yang isinya `Rc` — TIDAK `Send`).
///
/// Kenapa perlu tahap antara ini: `grid_to_plain_rows` dipanggil di
/// task tokio (thread background) waktu output SSH baru datang, tapi
/// `ModelRc`/`VecModel` cuma boleh dibuat & disentuh di thread UI. Jadi
/// alurnya: background thread bikin `PlainRow` (Send, aman dikirim
/// lewat `slint::invoke_from_event_loop`), lalu `plain_rows_to_slint`
/// BARU dipanggil di dalam closure itu (sudah di thread UI) buat
/// bungkus jadi `ModelRc<TermRow>` yang beneran di-`set_rows()`.
#[derive(Debug, Clone)]
pub struct PlainRun {
    pub text: String,
    pub fg: Rgb,
    pub bg: Rgb,
    pub bold: bool,
}

pub type PlainRow = Vec<PlainRun>;

fn to_slint_color(rgb: Rgb) -> slint::Color {
    slint::Color::from_rgb_u8(rgb.r, rgb.g, rgb.b)
}

/// Ubah satu snapshot grid jadi baris-baris "run" (rentetan sel
/// bersebelahan dengan fg/bg/bold yang sama digabung jadi satu),
/// supaya jumlah elemen UI yang perlu dirender jauh lebih sedikit
/// daripada satu elemen per sel (lihat komentar `TermRun` di
/// `ui/models.slint`). Aman dipanggil dari thread mana pun.
pub fn grid_to_plain_rows(grid: &TerminalGrid) -> Vec<PlainRow> {
    let cols = grid.cols as usize;
    let rows = grid.rows as usize;
    let (cursor_col, cursor_row) = (grid.cursor.0 as usize, grid.cursor.1 as usize);

    let mut out_rows: Vec<PlainRow> = Vec::with_capacity(rows);
    for row_idx in 0..rows {
        let mut runs: PlainRow = Vec::new();
        // (text-so-far, fg, bg, bold) buat run yang lagi dibangun.
        let mut current: Option<(String, Rgb, Rgb, bool)> = None;

        for col_idx in 0..cols {
            let cell = &grid.cells[row_idx * cols + col_idx];
            let is_cursor = grid.cursor_visible && row_idx == cursor_row && col_idx == cursor_col;
            let (fg, bg) = if is_cursor { (cell.bg, cell.fg) } else { (cell.fg, cell.bg) };

            match &mut current {
                Some((text, cfg, cbg, cbold)) if *cfg == fg && *cbg == bg && *cbold == cell.bold => {
                    text.push(cell.ch);
                }
                _ => {
                    if let Some((text, cfg, cbg, cbold)) = current.take() {
                        runs.push(PlainRun { text, fg: cfg, bg: cbg, bold: cbold });
                    }
                    current = Some((cell.ch.to_string(), fg, bg, cell.bold));
                }
            }
        }
        if let Some((text, cfg, cbg, cbold)) = current.take() {
            runs.push(PlainRun { text, fg: cfg, bg: cbg, bold: cbold });
        }
        out_rows.push(runs);
    }
    out_rows
}

/// Bungkus `PlainRow` (data murni) jadi `ModelRc<TermRow>` yang beneran
/// bisa di-`set_rows()` ke `ConsoleModel`. HARUS dipanggil di thread UI
/// (lihat komentar `PlainRun`) — jangan panggil ini dari task tokio.
pub fn plain_rows_to_slint(rows: Vec<PlainRow>) -> ModelRc<TermRow> {
    let out: Vec<TermRow> = rows
        .into_iter()
        .map(|runs| {
            let runs: Vec<TermRun> = runs
                .into_iter()
                .map(|r| TermRun { text: r.text.into(), fg: to_slint_color(r.fg), bg: to_slint_color(r.bg), bold: r.bold })
                .collect();
            TermRow { runs: ModelRc::new(VecModel::from(runs)) }
        })
        .collect();
    ModelRc::new(VecModel::from(out))
}

#[cfg(test)]
mod tests {
    use super::*;
    use terminus_term_emulator::TerminalInstance;

    #[test]
    fn teks_polos_jadi_run_yang_gabungannya_utuh() {
        let mut term = TerminalInstance::new(10, 2);
        term.feed(b"hi");
        let rows = grid_to_plain_rows(&term.snapshot(&terminus_term_emulator::palette::terminus_dark()));
        assert_eq!(rows.len(), 2);
        // Gabungan semua run di baris 0 harus balik jadi "hi" + 8 spasi
        // kosong. TIDAK dites "harus tepat 1 run" — kursor yang lagi
        // kelihatan di kolom 2 (persis setelah "hi") sengaja memecah
        // run jadi beberapa bagian lewat invert warna (lihat test
        // `kursor_invert_warna_di_posisinya`), itu perilaku yang benar.
        let joined: String = rows[0].iter().map(|r| r.text.as_str()).collect();
        assert_eq!(joined, "hi        ");
    }

    #[test]
    fn warna_beda_pecah_jadi_run_terpisah() {
        let mut term = TerminalInstance::new(10, 1);
        term.feed(b"\x1b[31mA\x1b[0mB");
        let rows = grid_to_plain_rows(&term.snapshot(&terminus_term_emulator::palette::terminus_dark()));
        // "A" (merah) beda warna dari "B" (default) -> run terpisah,
        // meski keduanya berdampingan tanpa spasi.
        assert!(rows[0].len() >= 2, "harus ada minimal 2 run: 'A' merah, sisanya default");
        assert_eq!(rows[0][0].text, "A");
        assert_eq!(rows[0][0].fg, terminus_term_emulator::grid::Rgb::new(0xcd, 0x31, 0x31));
    }

    #[test]
    fn kursor_invert_warna_di_posisinya() {
        let mut term = TerminalInstance::new(5, 1);
        term.feed(b"X");
        let mut snap = term.snapshot(&terminus_term_emulator::palette::terminus_dark());
        snap.cursor = (0, 0);
        snap.cursor_visible = true;
        let rows = grid_to_plain_rows(&snap);
        let first_run = &rows[0][0];
        // Di posisi kursor (kolom 0), fg/bg sel 'X' harus KETUKAR
        // dibanding sel lain yang tidak di-invert.
        let default_rows = grid_to_plain_rows(&term.snapshot(&terminus_term_emulator::palette::terminus_dark())); // tanpa cursor_visible
        let default_first = &default_rows[0][0];
        assert_eq!(first_run.fg, default_first.bg);
        assert_eq!(first_run.bg, default_first.fg);
    }

    #[test]
    fn plain_rows_to_slint_menghasilkan_jumlah_baris_yang_sama() {
        let mut term = TerminalInstance::new(10, 3);
        term.feed(b"a\r\nb\r\nc");
        let plain = grid_to_plain_rows(&term.snapshot(&terminus_term_emulator::palette::terminus_dark()));
        let model = plain_rows_to_slint(plain);
        assert_eq!(slint::Model::row_count(&model), 3);
    }
}
