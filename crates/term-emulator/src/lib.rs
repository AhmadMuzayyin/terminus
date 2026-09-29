//! `terminus-term-emulator`
//!
//! Parsing byte stream mentah dari SSH channel menjadi grid karakter +
//! atribut (warna, bold, cursor position, dsb) pakai VTE parser dari
//! project Alacritty. Crate ini TIDAK tahu apa-apa soal SSH maupun UI
//! toolkit — cuma terima `&[u8]` masuk, keluarkan struktur grid yang bisa
//! di-render oleh layer manapun (Slint di crate `app`).

use alacritty_terminal::event::VoidListener;
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::term::{Config as TermConfig, Term, TermMode};
use alacritty_terminal::vte::ansi::Processor;
use thiserror::Error;

pub mod grid;
pub mod palette;

pub use grid::TerminalGrid;

#[derive(Debug, Error)]
pub enum TermEmulatorError {
    #[error("gagal inisialisasi terminal state: {0}")]
    Init(String),
}

/// Ukuran terminal buat `Term::new`/`Term::resize` — `alacritty_terminal`
/// butuh implementasi trait `Dimensions`-nya sendiri, bukan tuple biasa.
/// `total_lines == screen_lines` di sini cuma ukuran LAYAR — riwayat
/// scrollback diatur terpisah oleh `TermConfig::scrolling_history`
/// (default alacritty 10.000 baris), lihat `scroll_view`.
struct Size {
    cols: usize,
    rows: usize,
}

impl Dimensions for Size {
    fn total_lines(&self) -> usize {
        self.rows
    }

    fn screen_lines(&self) -> usize {
        self.rows
    }

    fn columns(&self) -> usize {
        self.cols
    }
}

/// Instance satu terminal virtual (satu sesi SSH aktif = satu instance).
/// Membungkus `alacritty_terminal::Term` (state grid VT100/xterm) +
/// `Processor`-nya (parser byte mentah -> panggilan method di `Term`).
pub struct TerminalInstance {
    term: Term<VoidListener>,
    parser: Processor,
    cols: u16,
    rows: u16,
}

impl TerminalInstance {
    pub fn new(cols: u16, rows: u16) -> Self {
        let size = Size { cols: cols as usize, rows: rows as usize };
        let term = Term::new(TermConfig::default(), &size, VoidListener);
        Self { term, parser: Processor::new(), cols, rows }
    }

    /// Feed byte mentah yang baru datang dari SSH channel ke parser VTE.
    /// `parser` & `term` field terpisah (bukan lewat method `self.xxx()`)
    /// supaya borrow checker izinkan pinjam keduanya sekaligus di sini.
    pub fn feed(&mut self, bytes: &[u8]) {
        self.parser.advance(&mut self.term, bytes);
    }

    pub fn resize(&mut self, cols: u16, rows: u16) {
        self.cols = cols;
        self.rows = rows;
        self.term.resize(Size { cols: cols as usize, rows: rows as usize });
    }

    /// Geser tampilan ke riwayat scrollback: `lines` > 0 = naik (baris
    /// lebih lama), < 0 = turun. Otomatis di-clamp alacritty ke batas
    /// riwayat. Output baru yang datang selama tampilan digeser TIDAK
    /// menarik tampilan ke bawah (sama seperti terminal native).
    pub fn scroll_view(&mut self, lines: i32) {
        self.term.scroll_display(Scroll::Delta(lines));
    }

    /// Balik ke paling bawah (layar aktif). `true` kalau tadinya memang
    /// lagi digeser — caller cuma perlu render ulang di kasus itu.
    pub fn scroll_to_bottom(&mut self) -> bool {
        if self.display_offset() == 0 {
            return false;
        }
        self.term.scroll_display(Scroll::Bottom);
        true
    }

    /// Berapa baris tampilan lagi digeser ke atas (0 = paling bawah).
    pub fn display_offset(&self) -> usize {
        self.term.grid().display_offset()
    }

    /// Program layar penuh (`less`, `vim`, `htop`, `man`) jalan di
    /// "alternate screen" yang TIDAK punya scrollback — di situ roda
    /// mouse dikirim sebagai tombol panah (perilaku "alternate scroll"
    /// terminal native). `None` = bukan alternate screen, geser
    /// riwayat biasa lewat `scroll_view`.
    pub fn alternate_scroll_bytes(&self, lines: i32) -> Option<Vec<u8>> {
        let mode = self.term.mode();
        if !mode.contains(TermMode::ALT_SCREEN) || lines == 0 {
            return None;
        }
        let app_cursor = mode.contains(TermMode::APP_CURSOR);
        let seq: &[u8] = match (lines > 0, app_cursor) {
            (true, true) => b"\x1bOA",
            (true, false) => b"\x1b[A",
            (false, true) => b"\x1bOB",
            (false, false) => b"\x1b[B",
        };
        Some(seq.repeat(lines.unsigned_abs() as usize))
    }

    /// Snapshot grid saat ini untuk di-render UI. Warna sel diresolusi ke
    /// RGB konkret lewat `palette::resolve` (lihat modul itu buat
    /// kenapa perlu — `Term` sendiri cuma nyimpen warna abstrak).
    /// `theme` dioper di SETIAP panggilan (bukan disimpan di `self`)
    /// supaya ganti color theme di pengaturan terminal bisa langsung
    /// diterapkan ulang ke konten yang SUDAH ada (panggil `snapshot()`
    /// lagi dengan tema baru, tanpa perlu byte baru dari SSH) — lihat
    /// `crates/app/src/state.rs::on_terminal_theme_changed`.
    pub fn snapshot(&self, theme: &palette::Palette) -> TerminalGrid {
        use alacritty_terminal::term::cell::Flags;

        let mut out = TerminalGrid::empty(self.cols, self.rows);
        let content = self.term.renderable_content();
        let cursor_shape = content.cursor.shape;
        let cursor_point = content.cursor.point;
        let colors = content.colors;
        // Saat digeser ke riwayat, `display_iter` memberi nomor baris
        // negatif (-offset..) — digeser balik jadi 0-based layar.
        let offset = self.display_offset() as i32;

        for indexed in content.display_iter {
            let row = indexed.point.line.0 + offset;
            let col = indexed.point.column.0;
            if row < 0 || row as usize >= self.rows as usize || col >= self.cols as usize {
                continue; // di luar viewport
            }
            let idx = (row as usize) * (self.cols as usize) + col;
            let cell = &indexed.cell;
            let inverse = cell.flags.contains(Flags::INVERSE);
            let mut fg = palette::resolve(cell.fg, colors, theme);
            let mut bg = palette::resolve(cell.bg, colors, theme);
            if inverse {
                std::mem::swap(&mut fg, &mut bg);
            }
            out.cells[idx] = grid::Cell {
                ch: cell.c,
                fg,
                bg,
                bold: cell.flags.intersects(Flags::BOLD | Flags::DIM_BOLD),
                italic: cell.flags.contains(Flags::ITALIC),
                underline: cell.flags.intersects(Flags::ALL_UNDERLINES),
            };
        }

        // Kursor ikut bergeser; kalau sudah keluar layar (digeser jauh
        // ke atas) disembunyikan.
        let cursor_row = cursor_point.line.0 + offset;
        let cursor_on_screen = (0..self.rows as i32).contains(&cursor_row);
        out.cursor = (cursor_point.column.0 as u16, cursor_row.max(0) as u16);
        out.cursor_visible = cursor_on_screen && cursor_shape != alacritty_terminal::vte::ansi::CursorShape::Hidden;
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feed_teks_polos_muncul_di_snapshot() {
        let mut term = TerminalInstance::new(20, 5);
        term.feed(b"hello");
        let snap = term.snapshot(&palette::terminus_dark());
        let text: String = snap.cells[0..5].iter().map(|c| c.ch).collect();
        assert_eq!(text, "hello");
        // Kursor harus maju 5 kolom setelah 5 karakter dicetak.
        assert_eq!(snap.cursor, (5, 0));
    }

    #[test]
    fn feed_newline_pindah_baris() {
        let mut term = TerminalInstance::new(20, 5);
        term.feed(b"a\r\nb");
        let snap = term.snapshot(&palette::terminus_dark());
        assert_eq!(snap.cells[0].ch, 'a');
        assert_eq!(snap.cells[20].ch, 'b'); // baris ke-2, kolom ke-0
        assert_eq!(snap.cursor, (1, 1));
    }

    #[test]
    fn escape_sequence_warna_ansi_diresolusi_ke_rgb() {
        let mut term = TerminalInstance::new(20, 5);
        // SGR 31 = foreground merah ANSI standar (index 1 di palet default).
        term.feed(b"\x1b[31mX\x1b[0m");
        let snap = term.snapshot(&palette::terminus_dark());
        assert_eq!(snap.cells[0].ch, 'X');
        assert_eq!(snap.cells[0].fg, grid::Rgb::new(0xcd, 0x31, 0x31));
    }

    #[test]
    fn resize_mengubah_dimensi_grid() {
        let mut term = TerminalInstance::new(10, 5);
        term.resize(30, 10);
        let snap = term.snapshot(&palette::terminus_dark());
        assert_eq!((snap.cols, snap.rows), (30, 10));
    }

    /// Membuktikan mekanisme inti di balik fitur ganti color theme
    /// (`crates/app/src/state.rs::on_terminal_theme_changed`): konten
    /// yang SAMA (tidak di-`feed()` ulang) di-snapshot DUA KALI dengan
    /// tema BERBEDA harus menghasilkan RGB yang beda — tanpa ini,
    /// ganti tema tidak akan pernah kelihatan efeknya ke konten yang
    /// sudah ada di layar (cuma berlaku ke output berikutnya).
    #[test]
    fn snapshot_ulang_dengan_tema_beda_hasilkan_warna_beda() {
        let mut term = TerminalInstance::new(10, 1);
        term.feed(b"\x1b[31mX\x1b[0m"); // SGR 31 = merah ANSI standar

        let dark_fg = term.snapshot(&palette::terminus_dark()).cells[0].fg;
        let light_fg = term.snapshot(&palette::terminus_light()).cells[0].fg;
        let mono_fg = term.snapshot(&palette::mono_green()).cells[0].fg;

        assert_eq!(dark_fg, grid::Rgb::new(0xcd, 0x31, 0x31), "index merah tema Terminus Dark");
        assert_ne!(dark_fg, light_fg, "tema beda harus resolve ke RGB beda untuk index ANSI yang SAMA");
        assert_ne!(dark_fg, mono_fg);
        assert_ne!(light_fg, mono_fg);
    }

    fn first_row_text(term: &TerminalInstance) -> String {
        let snap = term.snapshot(&palette::terminus_dark());
        snap.cells[0..snap.cols as usize].iter().map(|c| c.ch).collect::<String>().trim_end().to_string()
    }

    #[test]
    fn scrollback_bisa_digeser_ke_atas_dan_balik_ke_bawah() {
        let mut term = TerminalInstance::new(10, 5);
        let output: Vec<String> = (0..50).map(|i| format!("L{i}")).collect();
        term.feed(output.join("\r\n").as_bytes());
        assert_eq!(first_row_text(&term), "L45", "layar aktif = 5 baris terakhir");

        term.scroll_view(3);
        assert_eq!(term.display_offset(), 3);
        assert_eq!(first_row_text(&term), "L42");

        // Geser jauh melewati awal riwayat -> di-clamp ke baris paling awal.
        term.scroll_view(1000);
        assert_eq!(first_row_text(&term), "L0");

        assert!(term.scroll_to_bottom());
        assert_eq!(first_row_text(&term), "L45");
        assert!(!term.scroll_to_bottom(), "sudah di bawah, tidak perlu render ulang");
    }

    #[test]
    fn kursor_disembunyikan_kalau_tergeser_keluar_layar() {
        let mut term = TerminalInstance::new(10, 3);
        term.feed((0..20).map(|i| format!("L{i}")).collect::<Vec<_>>().join("\r\n").as_bytes());
        assert!(term.snapshot(&palette::terminus_dark()).cursor_visible);
        term.scroll_view(10);
        assert!(!term.snapshot(&palette::terminus_dark()).cursor_visible);
    }

    #[test]
    fn alternate_screen_scroll_jadi_tombol_panah() {
        let mut term = TerminalInstance::new(10, 3);
        assert_eq!(term.alternate_scroll_bytes(2), None, "layar biasa -> geser riwayat, bukan panah");
        term.feed(b"\x1b[?1049h"); // masuk alternate screen (seperti less/vim)
        assert_eq!(term.alternate_scroll_bytes(2).unwrap(), b"\x1b[A\x1b[A");
        assert_eq!(term.alternate_scroll_bytes(-1).unwrap(), b"\x1b[B");
        term.feed(b"\x1b[?1h"); // application cursor keys
        assert_eq!(term.alternate_scroll_bytes(1).unwrap(), b"\x1bOA");
    }

    #[test]
    fn built_in_themes_semua_bisa_dicari_lewat_by_name() {
        for (name, expected) in palette::built_in_themes() {
            assert_eq!(palette::by_name(name), Some(expected), "by_name harus ketemu tema '{name}'");
        }
        assert_eq!(palette::by_name("Tema Yang Tidak Ada"), None);
    }
}
