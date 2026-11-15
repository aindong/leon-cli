//! What a pane shows when scrolled back: the live screen's rows below the scrollback's, by
//! absolute line number (`View::scrolled`), and which cells a selection or a match covers.

use std::collections::HashMap;

use leon_core::screen::{BgRect, Frame, Row, Span};
use leon_core::search::Pos;

/// A pane's rows: the frame to paint, and what is missing from the history cache.
pub struct Composed {
    pub frame: Frame,
    /// The absolute line of the top row.
    pub top: usize,
    /// History lines the view needs that aren't cached yet.
    pub missing: Vec<usize>,
}

/// The oldest line `live` keeps, and the top row at the bottom of the scrollback.
pub fn bounds(live: &Frame) -> (usize, usize) {
    (live.view.scrolled.saturating_sub(live.view.history), live.view.scrolled)
}

/// `top` kept inside the scrollback; None when that is the bottom (following the output). A
/// `pinned` view (copy mode, as in tmux) never follows: at the bottom, or None, it is the bottom.
pub fn clamp(live: &Frame, top: Option<usize>, pinned: bool) -> Option<usize> {
    let (oldest, bottom) = bounds(live);
    match pinned {
        true => Some(top.unwrap_or(bottom).clamp(oldest, bottom)),
        false => top.map(|t| t.clamp(oldest, bottom)).filter(|&t| t < bottom),
    }
}

/// The view from `top` (None: the live screen): rows at or past `scrolled` from the screen, older
/// ones from `cache`. The cursor shows only where its row is in view.
pub fn compose(live: &Frame, top: Option<usize>, cache: &HashMap<usize, Row>) -> Composed {
    let scrolled = live.view.scrolled;
    let Some(top) = clamp(live, top, false) else {
        return Composed { frame: live.clone(), top: scrolled, missing: Vec::new() };
    };
    let mut f = Frame { rows: vec![Vec::new(); live.lines], ..live.clone() };
    f.backgrounds.clear();
    let mut missing = Vec::new();
    // the screen's rects in one pass: screen line l shows on row l + scrolled - top
    let shift = scrolled - top;
    let shown = live.backgrounds.iter().filter(|b| b.line + shift < live.lines);
    f.backgrounds.extend(shown.map(|b| BgRect { line: b.line + shift, ..*b }));
    for r in 0..live.lines {
        let abs = top + r;
        if abs >= scrolled {
            f.rows[r] = live.rows.get(abs - scrolled).cloned().unwrap_or_default();
        } else if let Some(row) = cache.get(&abs) {
            f.rows[r] = row.spans.clone();
            let bgs = row.backgrounds.iter();
            f.backgrounds.extend(bgs.map(|b| BgRect { line: r, col: b.col, len: b.len, color: b.color }));
        } else {
            missing.push(abs);
        }
    }
    f.cursor = live.cursor.clone().and_then(|mut c| {
        c.line = (scrolled + c.line).checked_sub(top).filter(|&l| l < live.lines)?;
        Some(c)
    });
    Composed { frame: f, top, missing }
}

/// The columns of line `line` that the range `start..=end` covers (a `block` covers the same
/// columns on every line), in a screen `cols` wide.
pub fn covered(start: Pos, end: Pos, block: bool, line: usize, cols: usize) -> Option<(usize, usize)> {
    if line < start.line || line > end.line || cols == 0 {
        return None;
    }
    let last = cols - 1;
    if block {
        return Some((start.col.min(end.col).min(last), start.col.max(end.col).min(last)));
    }
    let from = if line == start.line { start.col } else { 0 };
    let to = if line == end.line { end.col } else { last };
    (from <= to).then_some((from.min(last), to.min(last)))
}

/// A row as one char per cell (a wide char's second cell is `\0`, combining marks dropped), for
/// reading columns back.
pub fn cells(spans: &[Span], cols: usize) -> Vec<char> {
    let mut out = vec![' '; cols];
    for s in spans {
        if s.cells == s.text.chars().count() {
            for (i, c) in s.text.chars().enumerate() {
                if let Some(cell) = out.get_mut(s.col + i) {
                    *cell = c;
                }
            }
        } else if let Some(c) = s.text.chars().next() {
            // a wide char, or a char with marks: its first char in its first cell (as far as the
            // row goes: `cells` comes from the server unchecked)
            for i in 0..s.cells.min(cols.saturating_sub(s.col)) {
                if let Some(cell) = out.get_mut(s.col + i) {
                    *cell = if i == 0 { c } else { '\0' };
                }
            }
        }
    }
    out
}

/// The text of columns `from..=to` of a row, trailing blanks dropped.
pub fn text_of(spans: &[Span], cols: usize, from: usize, to: usize) -> String {
    let cells = cells(spans, cols);
    let picked: String =
        cells.get(from..=to.min(cols.saturating_sub(1))).unwrap_or_default().iter().filter(|c| **c != '\0').collect();
    picked.trim_end().to_string()
}
