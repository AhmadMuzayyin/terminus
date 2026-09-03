/// Representasi grid karakter terminal yang platform-agnostic — tidak
/// terikat ke Slint atau toolkit UI manapun, supaya `term-emulator` tetap
/// bisa dites tanpa dependency UI.
#[derive(Debug, Clone)]
pub struct TerminalGrid {
    pub cols: u16,
    pub rows: u16,
    pub cells: Vec<Cell>,
    /// Posisi kursor sebagai `(kolom, baris)` — urutan ini (bukan
    /// baris-dulu) sengaja dipilih supaya konsisten dengan urutan
    /// pasangan `(x, y)` yang lazim.
    pub cursor: (u16, u16),
    /// Kursor kelihatan atau tidak (mis. mode `SHOW_CURSOR` OFF, atau
    /// sesi belum aktif) — konsumer (crate `app`) pakai ini buat
    /// memutuskan apakah perlu invert warna sel di posisi kursor.
    pub cursor_visible: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct Cell {
    pub ch: char,
    pub fg: Rgb,
    pub bg: Rgb,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

impl Default for Cell {
    fn default() -> Self {
        // Sel kosong = spasi, BUKAN `\0` (default bawaan `char`) — kalau
        // dipakai apa adanya bakal ke-render sebagai karakter NUL.
        Self { ch: ' ', fg: Rgb::default(), bg: Rgb::default(), bold: false, italic: false, underline: false }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

impl TerminalGrid {
    pub fn empty(cols: u16, rows: u16) -> Self {
        Self {
            cols,
            rows,
            cells: vec![Cell::default(); (cols as usize) * (rows as usize)],
            cursor: (0, 0),
            cursor_visible: false,
        }
    }
}
