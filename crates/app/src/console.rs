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

/// Ukuran grid AWAL, dipakai cuma sampai `TerminalSurface` melaporkan
/// ukuran sebenarnya yang muat di area terminal (`grid-resized`, lihat
/// `AppState.term_size`).
pub const DEFAULT_TERM_COLS: u16 = 100;
pub const DEFAULT_TERM_ROWS: u16 = 32;

/// Validasi ukuran dari UI (bisa 0/negatif sesaat waktu layout belum
/// jadi) — `None` = abaikan, jangan resize ke ukuran rusak.
pub fn valid_term_size(cols: i32, rows: i32) -> Option<(u16, u16)> {
    if cols < 2 || rows < 1 {
        return None;
    }
    Some((cols.min(1000) as u16, rows.min(500) as u16))
}

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

/// Teks polos tiap baris model yang lagi DITAMPILKAN (gabungan `text`
/// semua run) — sumber seleksi = persis apa yang user lihat.
pub fn slint_rows_to_lines(rows: &ModelRc<TermRow>) -> Vec<String> {
    use slint::Model;
    rows.iter().map(|row| row.runs.iter().map(|run| run.text.to_string()).collect()).collect()
}

/// Teks yang diseleksi di grid (koordinat sel, inklusif) — gaya
/// terminal: baris pertama dari `start_col` sampai ujung, baris tengah
/// penuh, baris terakhir sampai `end_col`. Spasi di ujung tiap baris
/// dibuang (sel kosong grid itu spasi, bukan isi yang disalin user).
/// Koordinat di luar grid di-clamp, urutan terbalik ditukar.
pub fn selection_text(lines: &[String], start: (i32, i32), end: (i32, i32)) -> String {
    if lines.is_empty() {
        return String::new();
    }
    let (start, end) = if start <= end { (start, end) } else { (end, start) };
    let last_line = lines.len() as i32 - 1;
    let first_row = start.0.clamp(0, last_line) as usize;
    let last_row = end.0.clamp(0, last_line) as usize;

    let mut out: Vec<String> = Vec::with_capacity(last_row - first_row + 1);
    for row in first_row..=last_row {
        let chars: Vec<char> = lines[row].chars().collect();
        let from = if row == first_row { start.1.max(0) as usize } else { 0 };
        let to = if row == last_row { (end.1.max(0) as usize + 1).min(chars.len()) } else { chars.len() };
        let piece: String = if from < to { chars[from..to].iter().collect() } else { String::new() };
        out.push(piece.trim_end().to_string());
    }
    out.join("\n")
}

/// Teks clipboard -> byte yang dikirim ke PTY/serial. Newline apa pun
/// (`\r\n`/`\n`) jadi `\r`, persis tombol Enter — shell di sisi server
/// yang menerjemahkan sendiri.
pub fn normalize_paste(text: &str) -> String {
    text.replace("\r\n", "\r").replace('\n', "\r")
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

    fn lines(raw: &[&str]) -> Vec<String> {
        raw.iter().map(|l| l.to_string()).collect()
    }

    #[test]
    fn seleksi_satu_baris_inklusif_dan_buang_spasi_ujung() {
        let grid = lines(&["$ ls -la   ", "total 0    "]);
        assert_eq!(selection_text(&grid, (0, 2), (0, 3)), "ls");
        // Seleksi sampai lewat ujung teks -> spasi sel kosong dibuang.
        assert_eq!(selection_text(&grid, (0, 2), (0, 50)), "ls -la");
    }

    #[test]
    fn seleksi_banyak_baris_gaya_terminal_dan_urutan_terbalik() {
        let grid = lines(&["abc def   ", "ghi jkl   ", "mno pqr   "]);
        let expected = "def\nghi jkl\nmno";
        assert_eq!(selection_text(&grid, (0, 4), (2, 2)), expected);
        // Drag dari bawah ke atas hasilnya harus sama.
        assert_eq!(selection_text(&grid, (2, 2), (0, 4)), expected);
    }

    #[test]
    fn seleksi_di_luar_grid_di_clamp() {
        let grid = lines(&["halo"]);
        assert_eq!(selection_text(&grid, (-3, -1), (9, 99)), "halo");
        assert_eq!(selection_text(&[], (0, 0), (0, 0)), "");
    }

    #[test]
    fn paste_newline_jadi_carriage_return() {
        assert_eq!(normalize_paste("ls\npwd\r\nwhoami"), "ls\rpwd\rwhoami");
    }
}
