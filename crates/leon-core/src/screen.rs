//! The render model: what a client paints for one terminal, built from alacritty's grid under its
//! lock and then painted without it. Rows are spans of same-styled text, each placed at its
//! start column, so glyphs stay on the cell grid whatever the font's fallback widths. The server
//! builds it and sends it to the app over the socket: a whole screen first, then the rows that
//! changed (`diff`, `apply`).

use alacritty_terminal::event::EventListener;
use alacritty_terminal::grid::{Dimensions, Indexed};
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::{Cell, Flags};
use alacritty_terminal::term::color::Colors;
use alacritty_terminal::term::{RenderableCursor, Term, TermMode};
use alacritty_terminal::vte::ansi::{Color, CursorShape, NamedColor, Rgb};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::mouse::Report;
use crate::terminal::MAX_SIZE;
use crate::theme::{indexed, mix, Theme};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
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
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BgRect {
    pub line: usize,
    pub col: usize,
    pub len: usize,
    pub color: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Cursor {
    pub line: usize,
    pub col: usize,
    /// `block`, `underline`, `beam`, `hollow` or `hidden`.
    #[serde(with = "shape")]
    #[schemars(with = "String")]
    pub shape: CursorShape,
    /// The cell is a wide char, so a block cursor covers two columns.
    pub wide: bool,
    /// What the cell holds, redrawn in the background color over a block cursor.
    pub text: String,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Frame {
    pub lines: usize,
    pub cols: usize,
    pub rows: Vec<Vec<Span>>,
    pub backgrounds: Vec<BgRect>,
    pub cursor: Option<Cursor>,
    /// The program asked for application cursor keys (DECCKM): arrows send `ESC O`.
    pub app_cursor: bool,
    /// The program asked for bracketed paste.
    pub bracketed_paste: bool,
    /// The scrolling state and the mouse modes (`View`).
    pub view: View,
}

/// Where the screen sits in the terminal's history, and what the program asked of the mouse.
#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct View {
    /// The absolute number of the screen's top row: how many lines were ever pushed into the
    /// scrollback. History line `scrolled - 1` is just above the screen; appended output never
    /// renumbers a line (docs/architecture/protocol-and-clients.md, "Scrollback in the app").
    pub scrolled: usize,
    /// Lines kept in the scrollback: the oldest is `scrolled - history`.
    pub history: usize,
    pub alt_screen: bool,
    /// `?1007`: on the alternate screen the wheel becomes arrow keys.
    pub alternate_scroll: bool,
    /// The mouse events the program asked for, if any.
    pub mouse: Option<Report>,
}

impl Frame {
    /// The screen as text, a line per row, spans placed at their columns.
    pub fn text(&self) -> String {
        self.rows.iter().map(|r| row_text(r) + "\n").collect()
    }
}

/// A row's text, its spans placed at their columns, trailing blanks dropped.
pub fn row_text(spans: &[Span]) -> String {
    let mut line = String::new();
    let mut at = 0;
    for s in spans {
        line.extend(std::iter::repeat_n(' ', s.col.saturating_sub(at)));
        line.push_str(&s.text);
        at = s.col + s.cells;
    }
    line.truncate(line.trim_end().len());
    line
}

/// One row of a screen update: its spans and its backgrounds (without the line).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Row {
    pub line: usize,
    pub spans: Vec<Span>,
    pub backgrounds: Vec<Bg>,
}

/// History rows by absolute number (`Terminal::lines`), with the numbering they were read under.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Lines {
    pub scrolled: usize,
    pub history: usize,
    pub rows: Vec<Row>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Bg {
    pub col: usize,
    pub len: usize,
    pub color: u32,
}

/// What a watcher is sent for one terminal: every row when `full` (the first screen, or a new
/// size), else only the rows that changed. The cursor and modes always come along.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Update {
    pub full: bool,
    pub lines: usize,
    pub cols: usize,
    pub rows: Vec<Row>,
    pub cursor: Option<Cursor>,
    pub app_cursor: bool,
    pub bracketed_paste: bool,
    #[serde(flatten)]
    pub view: View,
}

fn row(f: &Frame, line: usize) -> Row {
    let backgrounds =
        f.backgrounds.iter().filter(|b| b.line == line).map(|b| Bg { col: b.col, len: b.len, color: b.color });
    Row { line, spans: f.rows[line].clone(), backgrounds: backgrounds.collect() }
}

/// The update that turns `old` (None: nothing sent yet) into `new`; None when nothing changed.
pub fn diff(old: Option<&Frame>, new: &Frame) -> Option<Update> {
    let full = old.is_none_or(|o| (o.lines, o.cols) != (new.lines, new.cols));
    let rows: Vec<Row> = match old.filter(|_| !full) {
        None => (0..new.lines).map(|l| row(new, l)).collect(),
        Some(o) => (0..new.lines).map(|l| (row(o, l), row(new, l))).filter(|(a, b)| a != b).map(|(_, b)| b).collect(),
    };
    let same_rest = old.is_some_and(|o| {
        (&o.cursor, o.app_cursor, o.bracketed_paste, o.view)
            == (&new.cursor, new.app_cursor, new.bracketed_paste, new.view)
    });
    if !full && rows.is_empty() && same_rest {
        return None;
    }
    let f = new;
    let (cursor, app_cursor, bracketed_paste, view) = (f.cursor.clone(), f.app_cursor, f.bracketed_paste, f.view);
    Some(Update { full, lines: f.lines, cols: f.cols, rows, cursor, app_cursor, bracketed_paste, view })
}

/// Apply `u` to `frame`. Rows outside the screen are ignored, so a bad update can't grow it; a
/// line given twice takes its last row, and a row brings at most `cols` spans and backgrounds
/// inside the line, each span at most a cell's worth of text per column. Linear in the update and
/// the frame, whatever a server sends. shortcut: each update still scans the frame's backgrounds
/// (bounded at lines × cols, as the client's copy-on-write of the frame is); index them by line
/// if a profile ever shows it.
pub fn apply(frame: &mut Frame, u: Update) {
    if u.full {
        // no larger than a terminal can be, whatever the line said
        let (lines, cols) = (u.lines.min(MAX_SIZE.0 as usize), u.cols.min(MAX_SIZE.1 as usize));
        *frame = Frame { lines, cols, rows: vec![Vec::new(); lines], ..Frame::default() };
    }
    let mut rows: Vec<Option<Row>> = vec![None; frame.lines];
    for r in u.rows.into_iter().filter(|r| r.line < frame.lines) {
        let line = r.line;
        rows[line] = Some(r);
    }
    frame.backgrounds.retain(|b| rows[b.line].is_none());
    for r in rows.into_iter().flatten() {
        let line = r.line;
        let cols = frame.cols;
        let bgs = r.backgrounds.iter().filter(|b| b.col < cols).take(cols);
        frame.backgrounds.extend(bgs.map(|b| BgRect { line, col: b.col, len: b.len, color: b.color }));
        let mut spans: Vec<Span> = r.spans.into_iter().filter(|s| s.col < cols).take(cols).collect();
        for s in &mut spans {
            // a cell holds a char and its marks (`cell_span`): no more text than that per column
            let most = s.cells.clamp(1, cols) * (1 + MAX_MARKS);
            if let Some((cut, _)) = s.text.char_indices().nth(most) {
                s.text.truncate(cut);
            }
        }
        frame.rows[line] = spans;
    }
    (frame.cursor, frame.app_cursor, frame.bracketed_paste, frame.view) =
        (u.cursor, u.app_cursor, u.bracketed_paste, u.view);
}

/// Cursor shapes on the wire: `block`, `underline`, `beam`, `hollow` (alacritty's type has no serde).
mod shape {
    use alacritty_terminal::vte::ansi::CursorShape;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(s: &CursorShape, ser: S) -> Result<S::Ok, S::Error> {
        ser.serialize_str(match s {
            CursorShape::Block => "block",
            CursorShape::Underline => "underline",
            CursorShape::Beam => "beam",
            CursorShape::HollowBlock => "hollow",
            CursorShape::Hidden => "hidden",
        })
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<CursorShape, D::Error> {
        Ok(match String::deserialize(d)?.as_str() {
            "underline" => CursorShape::Underline,
            "beam" => CursorShape::Beam,
            "hollow" => CursorShape::HollowBlock,
            "hidden" => CursorShape::Hidden,
            _ => CursorShape::Block,
        })
    }
}

pub fn frame<T: EventListener>(term: &Term<T>, theme: &Theme) -> Frame {
    build(&Shot::of(term), theme)
}

/// What a frame is built from, copied out of the emulator under its lock: the parse thread then
/// waits only for the copy, not for the spans built from it.
pub struct Shot {
    lines: usize,
    cols: usize,
    cells: Vec<Indexed<Cell>>,
    offset: i32,
    cursor: RenderableCursor,
    colors: Colors,
    pub(crate) mode: TermMode,
    history: usize,
}

impl Shot {
    pub fn of<T: EventListener>(term: &Term<T>) -> Shot {
        let content = term.renderable_content();
        let (lines, cols) = (term.screen_lines(), term.columns());
        let mut cells = Vec::with_capacity(lines * cols);
        cells.extend(content.display_iter.map(|c| Indexed { point: c.point, cell: drawn(c.cell) }));
        Shot {
            lines,
            cols,
            cells,
            offset: content.display_offset as i32,
            cursor: content.cursor,
            colors: *content.colors,
            mode: content.mode,
            history: term.grid().history_size(),
        }
    }
}

/// `cell` as a frame draws it, sharing nothing with the emulator: a shared mark store would make
/// the emulator's next mark on the cell copy all of them (alacritty keeps any number).
fn drawn(cell: &Cell) -> Cell {
    let mut out = Cell { c: cell.c, fg: cell.fg, bg: cell.bg, flags: cell.flags, extra: None };
    let marks = cell.zerowidth().into_iter().flatten().copied().filter(|&c| !is_bidi_control(c));
    marks.take(MAX_MARKS).for_each(|m| out.push_zerowidth(m));
    out
}

/// The frame of `shot`, as `frame` builds it.
pub fn build(shot: &Shot, theme: &Theme) -> Frame {
    let (lines, cols) = (shot.lines, shot.cols);
    let mut f = Frame { lines, cols, rows: vec![Vec::new(); lines], ..Frame::default() };
    let offset = shot.offset;
    let cursor_at = shot.cursor.point;
    let mut cursor = (shot.cursor.shape != CursorShape::Hidden).then(|| Cursor {
        line: usize::MAX,
        col: cursor_at.column.0,
        shape: shot.cursor.shape,
        wide: false,
        text: String::new(),
    });
    for cell in &shot.cells {
        let Ok(line) = usize::try_from(cell.point.line.0 + offset) else { continue };
        if line >= lines {
            continue;
        }
        if cell.flags.intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER) {
            // A program can park the cursor on the spacer a wrapped wide char left behind.
            if let Some(c) = cursor.as_mut().filter(|_| cell.point == cursor_at) {
                (c.line, c.text) = (line, " ".into());
            }
            continue;
        }
        let (span, bg) = cell_span(cell, cell.point.column.0, &shot.colors, theme);
        if let Some(c) = cursor.as_mut().filter(|_| cell.point == cursor_at) {
            // Concealed text stays concealed under a block cursor too.
            let shown = if cell.flags.contains(Flags::HIDDEN) { " ".to_string() } else { span.text.clone() };
            (c.line, c.wide, c.text) = (line, span.cells == 2, shown);
        }
        push_cell(&mut f.rows[line], &mut f.backgrounds, line, span, bg, theme);
    }
    for r in &mut f.rows {
        trim(r);
    }
    f.cursor = cursor.filter(|c| c.line < lines);
    let mode = shot.mode;
    f.app_cursor = mode.contains(TermMode::APP_CURSOR);
    f.bracketed_paste = mode.contains(TermMode::BRACKETED_PASTE);
    f.view = View {
        history: shot.history,
        alt_screen: mode.contains(TermMode::ALT_SCREEN),
        alternate_scroll: mode.contains(TermMode::ALTERNATE_SCROLL),
        mouse: crate::mouse::report(mode, false),
        ..View::default()
    };
    f
}

/// The rows of grid lines `from..from + count` (alacritty's numbering: 0 is the screen's top,
/// history is negative), as `frame` builds a screen row; lines outside the grid are left out.
pub fn grid_rows<T>(term: &Term<T>, theme: &Theme, from: i32, count: usize) -> Vec<(i32, Vec<Span>, Vec<Bg>)> {
    let colors = term.colors();
    let (top, bottom) = (term.topmost_line().0, term.bottommost_line().0);
    let mut out = Vec::new();
    for l in (from.max(top)..from.saturating_add(count as i32).min(bottom + 1)).map(Line) {
        let row = &term.grid()[l];
        let (spans, bgs) = row_spans((0..term.columns()).map(|col| &row[Column(col)]), colors, theme);
        out.push((l.0, spans, bgs));
    }
    out
}

/// One row's spans and backgrounds from its cells (column 0 first), as `frame` builds a screen row.
pub fn row_spans<'a>(cells: impl Iterator<Item = &'a Cell>, colors: &Colors, theme: &Theme) -> (Vec<Span>, Vec<Bg>) {
    let (mut spans, mut bgs) = (Vec::new(), Vec::new());
    for (col, cell) in cells.enumerate() {
        if cell.flags.intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER) {
            continue;
        }
        let (span, bg) = cell_span(cell, col, colors, theme);
        push_cell(&mut spans, &mut bgs, 0, span, bg, theme);
    }
    trim(&mut spans);
    (spans, bgs.iter().map(|b| Bg { col: b.col, len: b.len, color: b.color }).collect())
}

/// One cell as a span, with its background.
fn cell_span(cell: &Cell, col: usize, colors: &Colors, theme: &Theme) -> (Span, u32) {
    let flags = cell.flags;
    let mut fg = resolve(cell.fg, colors, theme);
    let mut bg = resolve(cell.bg, colors, theme);
    if flags.contains(Flags::DIM) {
        fg = mix(fg, bg, DIM);
    }
    if flags.contains(Flags::INVERSE) {
        std::mem::swap(&mut fg, &mut bg);
    }
    if flags.contains(Flags::HIDDEN) {
        fg = bg;
    }
    let cells = if flags.contains(Flags::WIDE_CHAR) { 2 } else { 1 };
    let mut text = String::from(if cell.c == '\0' { ' ' } else { cell.c });
    // Bounded: a program can stack any number of marks on one cell, and each frame copies and shapes them.
    text.extend(cell.zerowidth().into_iter().flatten().copied().filter(|&c| !is_bidi_control(c)).take(MAX_MARKS));
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
    (span, bg)
}

/// Add a cell to its row: its background run, and its text joined to the last span when it can.
fn push_cell(row: &mut Vec<Span>, bgs: &mut Vec<BgRect>, line: usize, span: Span, bg: u32, theme: &Theme) {
    let (col, cells) = (span.col, span.cells);
    if bg != theme.bg {
        match bgs.last_mut() {
            Some(r) if r.line == line && r.col + r.len == col && r.color == bg => r.len += cells,
            _ => bgs.push(BgRect { line, col, len: cells, color: bg }),
        }
    }
    let blank = span.text == " " && !span.underline && !span.strikeout;
    // Spans hold one char per cell, so the painter can pin each glyph to its column. Wide chars
    // and cells with combining marks get spans of their own. A plain blank joins any span whose
    // style can't show on it (no underline or strikeout).
    let one_char = cells == 1 && span.text.chars().count() == 1;
    match row.last_mut() {
        Some(s)
            if one_char
                && s.col + s.cells == col
                // past 2 cells a span has been joined to, so it holds one char per cell: no count
                && (s.cells > 2 || s.cells == s.text.chars().count())
                && (same_style(s, &span) || blank && !s.underline && !s.strikeout) =>
        {
            s.text.push_str(&span.text);
            s.cells += 1;
        }
        _ if blank => {}
        _ => row.push(span),
    }
}

/// Plain blanks merged in at the end draw nothing; underlined or struck blanks do.
fn trim(row: &mut [Span]) {
    for s in row.iter_mut().filter(|s| !s.underline && !s.strikeout) {
        let trimmed = s.text.trim_end_matches(' ').len();
        s.cells -= s.text.len() - trimmed;
        s.text.truncate(trimmed);
    }
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

#[cfg(test)]
#[path = "screen_tests.rs"]
mod tests;
