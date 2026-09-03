//! Resolusi warna ANSI/xterm ke RGB konkret.
//!
//! `alacritty_terminal::Term` cuma nyimpen warna sel sebagai `Color`
//! abstrak (`Named`/`Indexed`/`Spec`) — belum tentu ada RGB eksplisit
//! kalau escape sequence-nya tidak pernah override lewat OSC 4/10/11.
//! `resolve()` yang isi kekosongan itu, pakai satu dari beberapa
//! `Palette` bawaan (color theme terminal, bisa dipilih user lewat
//! pengaturan di halaman Terminal — lihat `crates/app/src/state.rs`)
//! plus color cube 6x6x6 & grayscale ramp standar xterm-256color yang
//! SAMA di semua tema (bagian itu bukan bagian dari "tema").

use crate::grid::Rgb;
use alacritty_terminal::term::color::Colors as DynamicColors;
use alacritty_terminal::vte::ansi::{Color as VteColor, NamedColor};

/// Satu color theme terminal: background/foreground default + 16 warna
/// ANSI dasar. Dipilih user lewat pengaturan terminal (font & tema —
/// TIDAK ditampilkan permanen kayak referensi Termius, cuma muncul
/// waktu dibuka lewat tombol pengaturan di halaman Terminal).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    pub background: Rgb,
    pub foreground: Rgb,
    pub ansi: [Rgb; 16],
}

/// Default — senada tema gelap aplikasi (`Tokens.surface-container-
/// lowest`/`Tokens.on-surface` di `ui/tokens.slint`), 16 warna ANSI
/// ala VS Code (kontras bagus di atas background gelap, sudah dikenal
/// banyak orang).
pub fn terminus_dark() -> Palette {
    Palette {
        background: Rgb::new(0x06, 0x0e, 0x20),
        foreground: Rgb::new(0xda, 0xe2, 0xfd),
        ansi: [
            Rgb::new(0x00, 0x00, 0x00), // black
            Rgb::new(0xcd, 0x31, 0x31), // red
            Rgb::new(0x0d, 0xbc, 0x79), // green
            Rgb::new(0xe5, 0xe5, 0x10), // yellow
            Rgb::new(0x24, 0x72, 0xc8), // blue
            Rgb::new(0xbc, 0x3f, 0xbc), // magenta
            Rgb::new(0x11, 0xa8, 0xcd), // cyan
            Rgb::new(0xe5, 0xe5, 0xe5), // white
            Rgb::new(0x66, 0x66, 0x66), // bright black
            Rgb::new(0xf1, 0x4c, 0x4c), // bright red
            Rgb::new(0x23, 0xd1, 0x8b), // bright green
            Rgb::new(0xf5, 0xf5, 0x43), // bright yellow
            Rgb::new(0x3b, 0x8e, 0xea), // bright blue
            Rgb::new(0xd6, 0x70, 0xd6), // bright magenta
            Rgb::new(0x29, 0xb8, 0xdb), // bright cyan
            Rgb::new(0xff, 0xff, 0xff), // bright white
        ],
    }
}

/// Terang — buat yang lebih nyaman baca di background terang.
pub fn terminus_light() -> Palette {
    Palette {
        background: Rgb::new(0xfa, 0xfa, 0xfa),
        foreground: Rgb::new(0x1a, 0x1a, 0x1a),
        ansi: [
            Rgb::new(0x00, 0x00, 0x00), // black
            Rgb::new(0xc4, 0x1a, 0x1a), // red
            Rgb::new(0x0a, 0x8f, 0x5b), // green
            Rgb::new(0xa6, 0x86, 0x00), // yellow
            Rgb::new(0x1a, 0x56, 0xa6), // blue
            Rgb::new(0x9a, 0x2c, 0x9a), // magenta
            Rgb::new(0x0e, 0x82, 0x99), // cyan
            Rgb::new(0x4d, 0x4d, 0x4d), // white
            Rgb::new(0x76, 0x76, 0x76), // bright black
            Rgb::new(0xe0, 0x3c, 0x3c), // bright red
            Rgb::new(0x1c, 0xb3, 0x73), // bright green
            Rgb::new(0xc7, 0xa1, 0x0a), // bright yellow
            Rgb::new(0x3b, 0x82, 0xe0), // bright blue
            Rgb::new(0xc2, 0x4f, 0xc2), // bright magenta
            Rgb::new(0x2c, 0xa6, 0xc2), // bright cyan
            Rgb::new(0x1a, 0x1a, 0x1a), // bright white
        ],
    }
}

/// Biru gelap pekat — kontras tinggi, aksen biru terang.
pub fn midnight_blue() -> Palette {
    Palette {
        background: Rgb::new(0x0a, 0x12, 0x28),
        foreground: Rgb::new(0xd6, 0xe6, 0xff),
        ansi: [
            Rgb::new(0x0a, 0x12, 0x28),
            Rgb::new(0xff, 0x5c, 0x5c),
            Rgb::new(0x4c, 0xd6, 0x8c),
            Rgb::new(0xff, 0xd1, 0x66),
            Rgb::new(0x4f, 0x9c, 0xff),
            Rgb::new(0xc4, 0x7a, 0xff),
            Rgb::new(0x4f, 0xd6, 0xe0),
            Rgb::new(0xc9, 0xd8, 0xf0),
            Rgb::new(0x50, 0x5f, 0x82),
            Rgb::new(0xff, 0x8a, 0x8a),
            Rgb::new(0x7c, 0xe8, 0xab),
            Rgb::new(0xff, 0xe0, 0x99),
            Rgb::new(0x87, 0xbb, 0xff),
            Rgb::new(0xd9, 0xa8, 0xff),
            Rgb::new(0x87, 0xe8, 0xf0),
            Rgb::new(0xff, 0xff, 0xff),
        ],
    }
}

/// Hijau monokrom ala terminal jadul — cuma satu hue, beda kecerahan.
pub fn mono_green() -> Palette {
    let bg = Rgb::new(0x03, 0x0a, 0x03);
    let dim = Rgb::new(0x1f, 0x7a, 0x1f);
    let normal = Rgb::new(0x33, 0xd6, 0x33);
    let bright = Rgb::new(0x6b, 0xff, 0x6b);
    Palette {
        background: bg,
        foreground: normal,
        ansi: [bg, dim, normal, normal, dim, dim, normal, normal, dim, bright, bright, bright, bright, bright, bright, bright],
    }
}

/// Amber monokrom — sama konsepnya dengan `mono_green`, hue kuning-oranye.
pub fn mono_amber() -> Palette {
    let bg = Rgb::new(0x0a, 0x07, 0x02);
    let dim = Rgb::new(0x8a, 0x5a, 0x10);
    let normal = Rgb::new(0xdb, 0x9a, 0x1f);
    let bright = Rgb::new(0xff, 0xc4, 0x5c);
    Palette {
        background: bg,
        foreground: normal,
        ansi: [bg, dim, normal, normal, dim, dim, normal, normal, dim, bright, bright, bright, bright, bright, bright, bright],
    }
}

/// Hangat, kontras sedang — hue oranye/merah dominan (terinspirasi
/// palet-palet "sunset").
pub fn solar_flare() -> Palette {
    Palette {
        background: Rgb::new(0x1b, 0x0f, 0x0a),
        foreground: Rgb::new(0xff, 0xe4, 0xc4),
        ansi: [
            Rgb::new(0x1b, 0x0f, 0x0a),
            Rgb::new(0xe0, 0x4b, 0x2c),
            Rgb::new(0x8f, 0xb3, 0x3d),
            Rgb::new(0xe0, 0xa6, 0x2c),
            Rgb::new(0xd9, 0x6a, 0x2c),
            Rgb::new(0xc2, 0x3c, 0x6e),
            Rgb::new(0xe0, 0x8a, 0x2c),
            Rgb::new(0xe8, 0xc9, 0xa8),
            Rgb::new(0x6b, 0x4a, 0x3a),
            Rgb::new(0xff, 0x6e, 0x4d),
            Rgb::new(0xb5, 0xe0, 0x5c),
            Rgb::new(0xff, 0xc6, 0x4d),
            Rgb::new(0xff, 0x8f, 0x4d),
            Rgb::new(0xe0, 0x7a, 0x9c),
            Rgb::new(0xff, 0xad, 0x4d),
            Rgb::new(0xff, 0xf3, 0xe0),
        ],
    }
}

/// Semua tema bawaan, urut sesuai kemunculan di UI pemilih tema.
/// `(nama, palet)` — nama ini yang dikirim bolak-balik lewat
/// `TerminalTabsModel.terminal-theme-name`/`terminal-theme-changed`.
pub fn built_in_themes() -> Vec<(&'static str, Palette)> {
    vec![
        ("Terminus Dark", terminus_dark()),
        ("Terminus Light", terminus_light()),
        ("Midnight Blue", midnight_blue()),
        ("Solar Flare", solar_flare()),
        ("Mono Green", mono_green()),
        ("Mono Amber", mono_amber()),
    ]
}

/// Cari tema built-in berdasarkan nama persis (case-sensitive, sesuai
/// yang dikirim UI). `None` kalau nama tidak dikenali — caller
/// (`state.rs`) fallback ke `terminus_dark()`.
pub fn by_name(name: &str) -> Option<Palette> {
    built_in_themes().into_iter().find(|(n, _)| *n == name).map(|(_, p)| p)
}

fn indexed(idx: u8, palette: &Palette) -> Rgb {
    match idx {
        0..=15 => palette.ansi[idx as usize],
        // 16..=231: kubus warna 6x6x6 standar xterm-256color — SAMA di
        // semua tema, bukan bagian yang di-kustomisasi.
        16..=231 => {
            let i = idx - 16;
            let r = i / 36;
            let g = (i % 36) / 6;
            let b = i % 6;
            const STEP: [u8; 6] = [0x00, 0x5f, 0x87, 0xaf, 0xd7, 0xff];
            Rgb::new(STEP[r as usize], STEP[g as usize], STEP[b as usize])
        }
        // 232..=255: grayscale ramp standar xterm-256color.
        _ => {
            let level = 8 + (idx - 232) * 10;
            Rgb::new(level, level, level)
        }
    }
}

fn named(color: NamedColor, palette: &Palette) -> Rgb {
    use NamedColor::*;
    match color {
        Black | DimBlack => palette.ansi[0],
        Red | DimRed => palette.ansi[1],
        Green | DimGreen => palette.ansi[2],
        Yellow | DimYellow => palette.ansi[3],
        Blue | DimBlue => palette.ansi[4],
        Magenta | DimMagenta => palette.ansi[5],
        Cyan | DimCyan => palette.ansi[6],
        White | DimWhite => palette.ansi[7],
        BrightBlack => palette.ansi[8],
        BrightRed => palette.ansi[9],
        BrightGreen => palette.ansi[10],
        BrightYellow => palette.ansi[11],
        BrightBlue => palette.ansi[12],
        BrightMagenta => palette.ansi[13],
        BrightCyan => palette.ansi[14],
        BrightWhite => palette.ansi[15],
        Foreground | BrightForeground | DimForeground => palette.foreground,
        Background => palette.background,
        Cursor => palette.foreground,
    }
}

/// Resolusi satu `Color` sel jadi RGB konkret. `dynamic` adalah palet
/// yang mungkin sudah di-override runtime lewat escape sequence OSC —
/// dicek duluan sebelum jatuh ke `palette` (tema yang lagi dipilih
/// user) sebagai default.
pub fn resolve(color: VteColor, dynamic: &DynamicColors, palette: &Palette) -> Rgb {
    match color {
        VteColor::Spec(rgb) => Rgb::new(rgb.r, rgb.g, rgb.b),
        VteColor::Indexed(idx) => {
            if let Some(rgb) = dynamic[idx as usize] {
                Rgb::new(rgb.r, rgb.g, rgb.b)
            } else {
                indexed(idx, palette)
            }
        }
        VteColor::Named(n) => {
            if let Some(rgb) = dynamic[n as usize] {
                Rgb::new(rgb.r, rgb.g, rgb.b)
            } else {
                named(n, palette)
            }
        }
    }
}
