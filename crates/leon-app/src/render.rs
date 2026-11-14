//! The render model: what the view paints for one terminal, built from alacritty's grid under its
//! lock and then painted without it. Rows are spans of same-styled text, each placed at its
//! start column, so glyphs stay on the cell grid whatever the font's fallback widths.

use alacritty_terminal::event::EventListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::color::Colors;
use alacritty_terminal::term::Term;
use alacritty_terminal::vte::ansi::{Color, CursorShape, NamedColor, Rgb};

use crate::theme::{indexed, mix, Theme};

#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    pub col: usize,
    /// Columns covered (a wide char counts two, combining marks none).
    pub cells: usize,
    pub text: String,
    pub fg: u32,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikeout: bool,
}

/// A run of cells on one line with a non-default background.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BgRect {
    pub line: usize,
    pub col: usize,
    pub len: usize,
    pub color: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Cursor {
    pub line: usize,
    pub col: usize,
    pub shape: CursorShape,
    /// The cell is a wide char, so a block cursor covers two columns.
    pub wide: bool,
    /// What the cell holds, redrawn in the background color over a block cursor.
    pub text: String,
}

#[derive(Debug, Default)]
pub struct Frame {
    pub lines: usize,
    pub cols: usize,
    pub rows: Vec<Vec<Span>>,
    pub backgrounds: Vec<BgRect>,
    pub cursor: Option<Cursor>,
}

pub fn frame<T: EventListener>(term: &Term<T>, theme: &Theme) -> Frame {
    let content = term.renderable_content();
    let (lines, cols) = (term.screen_lines(), term.columns());
    let mut f = Frame { lines, cols, rows: vec![Vec::new(); lines], ..Frame::default() };
    let offset = content.display_offset as i32;
    let cursor_at = content.cursor.point;
    let mut cursor = (content.cursor.shape != CursorShape::Hidden).then(|| Cursor {
        line: usize::MAX,
        col: cursor_at.column.0,
        shape: content.cursor.shape,
        wide: false,
        text: String::new(),
    });
    for cell in content.display_iter {
        let Ok(line) = usize::try_from(cell.point.line.0 + offset) else { continue };
        if line >= lines {
            continue;
        }
        let col = cell.point.column.0;
        let flags = cell.flags;
        if flags.intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER) {
            // A program can park the cursor on the spacer a wrapped wide char left behind.
            if let Some(c) = cursor.as_mut().filter(|_| cell.point == cursor_at) {
                (c.line, c.text) = (line, " ".into());
            }
            continue;
        }
        let mut fg = resolve(cell.fg, content.colors, theme);
        let mut bg = resolve(cell.bg, content.colors, theme);
        if flags.contains(Flags::DIM) {
            fg = mix(fg, bg, DIM);
        }
        if flags.contains(Flags::INVERSE) {
            std::mem::swap(&mut fg, &mut bg);
        }
        if flags.contains(Flags::HIDDEN) {
            fg = bg;
        }
        let wide = flags.contains(Flags::WIDE_CHAR);
        let cells = if wide { 2 } else { 1 };
        if bg != theme.bg {
            match f.backgrounds.last_mut() {
                Some(r) if r.line == line && r.col + r.len == col && r.color == bg => r.len += cells,
                _ => f.backgrounds.push(BgRect { line, col, len: cells, color: bg }),
            }
        }
        let mut text = String::from(if cell.c == '\0' { ' ' } else { cell.c });
        // Bounded: a program can stack any number of marks on one cell, and each frame copies and shapes them.
        text.extend(cell.zerowidth().into_iter().flatten().copied().filter(|&c| !is_bidi_control(c)).take(MAX_MARKS));
        if let Some(c) = cursor.as_mut().filter(|_| cell.point == cursor_at) {
            // Concealed text stays concealed under a block cursor too.
            let shown = if flags.contains(Flags::HIDDEN) { " ".to_string() } else { text.clone() };
            (c.line, c.wide, c.text) = (line, wide, shown);
        }
        let span = Span {
            col,
            cells,
            text,
            fg,
            bold: flags.contains(Flags::BOLD),
            italic: flags.contains(Flags::ITALIC),
            underline: flags.intersects(Flags::ALL_UNDERLINES),
            strikeout: flags.contains(Flags::STRIKEOUT),
        };
        let blank = span.text == " " && !span.underline && !span.strikeout;
        let row = &mut f.rows[line];
        // Spans hold one char per cell, so the painter can pin each glyph to its column. Wide chars
        // and cells with combining marks get spans of their own. A plain blank joins any span whose
        // style can't show on it (no underline or strikeout).
        let one_char = !wide && span.text.chars().count() == 1;
        match row.last_mut() {
            Some(s)
                if one_char
                    && s.col + s.cells == col
                    && s.cells == s.text.chars().count()
                    && (same_style(s, &span) || blank && !s.underline && !s.strikeout) =>
            {
                s.text.push_str(&span.text);
                s.cells += 1;
            }
            _ if blank => {}
            _ => row.push(span),
        }
    }
    for row in &mut f.rows {
        // Plain blanks merged in at the end draw nothing; underlined or struck blanks do.
        for s in row.iter_mut().filter(|s| !s.underline && !s.strikeout) {
            let trimmed = s.text.trim_end_matches(' ').len();
            s.cells -= s.text.len() - trimmed;
            s.text.truncate(trimmed);
        }
    }
    f.cursor = cursor.filter(|c| c.line < lines);
    f
}

/// Combining marks drawn per cell.
const MAX_MARKS: usize = 16;
/// How far dim text fades toward the background.
const DIM: f32 = 0.35;

/// Bidi controls would let the shaper reorder text away from the cells it sits in.
fn is_bidi_control(c: char) -> bool {
    matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}' | '\u{200e}' | '\u{200f}')
}

fn same_style(a: &Span, b: &Span) -> bool {
    (a.fg, a.bold, a.italic, a.underline, a.strikeout) == (b.fg, b.bold, b.italic, b.underline, b.strikeout)
}

fn rgb(c: Rgb) -> u32 {
    (c.r as u32) << 16 | (c.g as u32) << 8 | c.b as u32
}

/// A cell color: the program's palette overrides (OSC 4/10/11) first, then the theme.
fn resolve(c: Color, colors: &Colors, theme: &Theme) -> u32 {
    match c {
        Color::Spec(c) => rgb(c),
        Color::Indexed(i) => colors[i as usize].map_or_else(|| indexed(i, theme), rgb),
        Color::Named(n) => colors[n].map_or_else(|| named(n, theme), rgb),
    }
}

fn named(n: NamedColor, theme: &Theme) -> u32 {
    use NamedColor::*;
    match n {
        Foreground | BrightForeground => theme.fg,
        Background => theme.bg,
        Cursor => theme.cursor,
        DimForeground => mix(theme.fg, theme.bg, DIM),
        DimBlack | DimRed | DimGreen | DimYellow | DimBlue | DimMagenta | DimCyan | DimWhite => {
            mix(theme.ansi[n as usize - DimBlack as usize], theme.bg, DIM)
        }
        _ => theme.ansi[n as usize],
    }
}
