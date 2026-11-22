//! The web page's QR code (docs/web.md): one encoder for the Settings window, which paints the
//! modules, and `leon server status --qr`, which prints them as half blocks.

use leon_core::theme::{contrast, Theme};
use qrcode::{Color, QrCode};

/// The light margin a scanner needs around the code, in modules.
pub const QUIET: usize = 4;
/// The least contrast between the background and title for the code to use them.
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

/// The code's (field, module) colours from the theme: its background and title, the lighter one
/// the field, as a scanner wants dark modules on light. If those two are close, the pair of the
/// theme's own colours with the most contrast (its terminal white and black, say).
pub fn colours(t: &Theme) -> (u32, u32) {
    let pairs = [(t.bg, t.title), (t.bg, t.fg), (t.chrome, t.text), (t.ansi[15], t.ansi[0]), (t.ansi[7], t.ansi[0])];
    let (a, b) = match contrast(t.bg, t.title) >= MIN_CONTRAST {
        true => pairs[0],
        false => pairs.into_iter().max_by(|x, y| contrast(x.0, x.1).total_cmp(&contrast(y.0, y.1))).unwrap_or(pairs[0]),
    };
    // relative to black: the lighter colour has more contrast with it
    if contrast(a, 0) >= contrast(b, 0) {
        (a, b)
    } else {
        (b, a)
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
