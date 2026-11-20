//! The web page's QR code (docs/web.md): one encoder for the Settings window, which paints the
//! modules, and `leon server status --qr`, which prints them as half blocks.

use leon_core::theme::contrast;
use qrcode::{Color, QrCode};

/// The light margin a scanner needs around the code, in modules.
pub const QUIET: usize = 4;
/// The least contrast between the code's two colours: below it, black on white.
pub const MIN_CONTRAST: f64 = 7.;

/// `text`'s QR code as rows of modules (true: dark), without the quiet zone; None if too long.
pub fn modules(text: &str) -> Option<Vec<Vec<bool>>> {
    let code = QrCode::new(text).ok()?;
    let w = code.width();
    Some(code.to_colors().chunks(w).map(|row| row.iter().map(|c| *c == Color::Dark).collect()).collect())
}

/// `text`'s QR code with its quiet zone, two module rows a line: a light module is drawn (`▀`,
/// `▄`, `█`) and a dark one left blank, so it reads light on dark, as a terminal draws text.
pub fn half_blocks(text: &str) -> Option<Vec<String>> {
    let m = modules(text)?;
    let side = m.len() + 2 * QUIET;
    let light = |x: usize, y: usize| {
        let inside = |v: usize| v >= QUIET && v < QUIET + m.len();
        !(inside(x) && inside(y) && m[y - QUIET][x - QUIET])
    };
    let lines = (0..side.div_ceil(2)).map(|r| {
        (0..side)
            .map(|x| match (light(x, 2 * r), 2 * r + 1 < side && light(x, 2 * r + 1)) {
                (true, true) => '█',
                (true, false) => '▀',
                (false, true) => '▄',
                (false, false) => ' ',
            })
            .collect()
    });
    Some(lines.collect())
}

/// The code's (field, module) colours from a theme's background and title colours: the lighter
/// one is the field, as a scanner wants dark modules on light; black on white if they are close.
pub fn colours(bg: u32, title: u32) -> (u32, u32) {
    if contrast(bg, title) < MIN_CONTRAST {
        return (0xFFFFFF, 0x000000);
    }
    if contrast(bg, 0) > contrast(title, 0) {
        (bg, title)
    } else {
        (title, bg)
    }
}

/// The dark modules as runs, (row, first column, length), each a quad the window paints.
pub fn runs(m: &[Vec<bool>]) -> Vec<(usize, usize, usize)> {
    let mut out = vec![];
    for (y, row) in m.iter().enumerate() {
        let mut x = 0;
        while x < row.len() {
            let len = row[x..].iter().take_while(|d| **d).count();
            if len > 0 {
                out.push((y, x, len));
            }
            x += len.max(1);
        }
    }
    out
}

/// `half_blocks` for a terminal: with `colour`, each line white on black, so the code is the
/// right way round whatever the terminal's own colours.
pub fn terminal_lines(text: &str, colour: bool) -> Option<Vec<String>> {
    let lines = half_blocks(text)?;
    Some(if colour { lines.into_iter().map(|l| format!("\x1b[97;40m{l}\x1b[0m")).collect() } else { lines })
}
