//! leon's store of history (docs/architecture/scrollback.md). alacritty keeps every history row at
//! the pane's full width, 24 bytes a cell; so when output pauses its history moves here as each
//! row's text and runs of style, about what the text costs. The store's rows are numbered just
//! above alacritty's oldest, so a line keeps its number as it moves.

use std::collections::VecDeque;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use alacritty_terminal::grid::{Dimensions, GridCell, Row};
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::{Cell, CellExtra, Flags};
use alacritty_terminal::term::{Term, TermMode};
use alacritty_terminal::vte::ansi::{Color, NamedColor};

use super::{lock, Listener, Terminal, MAX_SCROLLBACK};

/// The history rows alacritty keeps once output pauses (`settle`): its storage then holds the
/// screen plus at most these (it allocates history 1,000 rows at a time).
pub const WINDOW: usize = 1000;
/// Stored rows are allocated this many at a time.
const GROW: usize = 1024;

/// The emulator and the store, under one lock: a reader sees both as one history.
pub struct Emu {
    pub(super) term: Term<Listener>,
    pub(super) store: Store,
}

impl Deref for Emu {
    type Target = Term<Listener>;
    fn deref(&self) -> &Term<Listener> {
        &self.term
    }
}

impl DerefMut for Emu {
    fn deref_mut(&mut self) -> &mut Term<Listener> {
        &mut self.term
    }
}

impl Emu {
    /// The lines of history kept, the store's and alacritty's; none on the alternate screen,
    /// whose own grid has no history.
    pub fn history(&self) -> usize {
        if alt(&self.term) {
            0
        } else {
            self.store.rows.len() + self.term.grid().history_size()
        }
    }

    /// Stored lines (oldest first), shown above alacritty's history; none on the alternate screen.
    pub(super) fn stored(&self) -> usize {
        if alt(&self.term) {
            0
        } else {
            self.store.rows.len()
        }
    }
}

fn alt(term: &Term<Listener>) -> bool {
    term.mode().contains(TermMode::ALT_SCREEN)
}

impl Terminal {
    /// Keep at most `lines` of history (at most `MAX_SCROLLBACK`): new output is capped at once,
    /// and what is kept past it now goes.
    pub fn set_scrollback(&self, lines: usize) {
        {
            let mut emu = lock(&self.term);
            let Emu { term, store } = &mut *emu;
            store.cap = lines.min(MAX_SCROLLBACK);
            store.settle(term);
            store.trim(term);
        }
        self.generation.send_modify(|g| *g += 1);
    }
}

/// A run of cells with one style: the store keeps a row as its characters and these.
#[derive(Clone, Copy)]
struct Run {
    cells: u16,
    flags: Flags,
    fg: Color,
    bg: Color,
}

/// One row: a character per cell up to its last one that shows (a character, or a blank with a
/// colour or a line), its runs (none for a row all in the default style), and the few cells with
/// combining marks, an underline colour or an OSC 8 link (alacritty's `CellExtra`, shared with
/// the cells it came from).
struct Stored {
    text: Box<str>,
    runs: Option<Box<[Run]>>,
    extras: Box<[(u16, Arc<CellExtra>)]>,
}

const PLAIN: Run = Run {
    cells: 0,
    flags: Flags::empty(),
    fg: Color::Named(NamedColor::Foreground),
    bg: Color::Named(NamedColor::Background),
};

pub struct Store {
    rows: VecDeque<Stored>,
    /// The most lines of history kept, the store's and alacritty's (`scrollback`).
    pub(super) cap: usize,
}

impl Store {
    pub(super) fn new(cap: usize) -> Store {
        Store { rows: VecDeque::new(), cap }
    }

    pub(super) fn clear(&mut self) {
        self.rows = VecDeque::new();
    }

    fn push(&mut self, row: &Row<Cell>) {
        // alacritty's text ends at the last character (`LineLength`); a coloured blank after it
        // still draws
        let len = row.into_iter().rposition(|c| c.c != ' ' || !c.is_empty()).map_or(0, |i| i + 1);
        let mut text = String::with_capacity(len);
        let (mut runs, mut extras): (Vec<Run>, Vec<_>) = (Vec::new(), Vec::new());
        for col in 0..len {
            let c = &row[Column(col)];
            text.push(c.c);
            match runs.last_mut() {
                Some(r) if (r.fg, r.bg, r.flags) == (c.fg, c.bg, c.flags) => r.cells += 1,
                _ => runs.push(Run { cells: 1, flags: c.flags, fg: c.fg, bg: c.bg }),
            }
            if let Some(e) = &c.extra {
                extras.push((col as u16, e.clone()));
            }
        }
        // grown a block at a time, not doubled: the store is most of a pane's history
        if self.rows.len() == self.rows.capacity() {
            self.rows.reserve_exact(GROW);
        }
        let plain = runs.iter().all(|r| (r.flags, r.fg, r.bg) == (PLAIN.flags, PLAIN.fg, PLAIN.bg));
        let runs = (!plain).then(|| runs.into());
        self.rows.push_back(Stored { text: text.into(), runs, extras: extras.into() });
    }

    /// Stored row `i`'s cells as alacritty had them, trailing blanks left out.
    pub(super) fn cells(&self, i: usize) -> Vec<Cell> {
        let Some(row) = self.rows.get(i) else { return Vec::new() };
        let mut chars = row.text.chars();
        let mut out = Vec::with_capacity(row.text.len());
        let plain = [Run { cells: u16::MAX, ..PLAIN }];
        for r in row.runs.as_deref().unwrap_or(&plain) {
            for c in chars.by_ref().take(r.cells.into()) {
                out.push(Cell { c, fg: r.fg, bg: r.bg, flags: r.flags, extra: None });
            }
        }
        for (col, e) in row.extras.iter() {
            out[usize::from(*col)].extra = Some(e.clone());
        }
        out
    }

    /// Whether stored row `i` wrapped: its line goes on in the next row.
    pub(super) fn wrapped(&self, i: usize) -> bool {
        // a wrapped row's last cell has WRAPLINE, so its runs aren't the plain ones
        let last = self.rows.get(i).and_then(|r| r.runs.as_deref()?.last());
        last.is_some_and(|r| r.flags.contains(Flags::WRAPLINE))
    }

    /// Every stored row's text, as alacritty's `bounds_to_string` gives a whole row: no newline
    /// after a wrapped row, a tab's blanks skipped. shortcut: tab stops are the default 8
    /// columns (alacritty doesn't expose a program's own, set with HTS); read them if one matters.
    pub(super) fn text(&self) -> String {
        let mut text = String::new();
        for i in 0..self.rows.len() {
            let cells = self.cells(i);
            // `LineLength` for a row: through a wrapped row's last cell, else its last character
            let shown = |c: &Cell| c.c != ' ' || c.zerowidth().is_some_and(|z| !z.is_empty());
            let len = match cells.last() {
                Some(c) if c.flags.contains(Flags::WRAPLINE) => cells.len(),
                _ => cells.iter().rposition(shown).map_or(0, |i| i + 1),
            };
            let mut tab = false;
            for (col, cell) in cells[..len].iter().enumerate() {
                if tab && col % 8 != 0 && cell.c == ' ' {
                    continue;
                }
                tab = cell.c == '\t';
                if !cell.flags.intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER) {
                    text.push(cell.c);
                    text.extend(cell.zerowidth().into_iter().flatten());
                }
            }
            if len == 0 || !cells[len - 1].flags.contains(Flags::WRAPLINE) {
                text.push('\n');
            }
        }
        text
    }

    /// alacritty's history limit: the cap, so any row it drops is one the cap drops too (the
    /// store is older still, and `trim` empties it then).
    pub(super) fn limit(&self) -> usize {
        self.cap.max(1)
    }

    /// When output pauses, on the primary screen: once alacritty holds more than its window, all
    /// of its history moves here, which frees the storage it grew for it. While output streams it
    /// keeps every row at full width, as alacritty alone does, and none is copied.
    pub(super) fn settle(&mut self, term: &mut Term<Listener>) {
        if !alt(term) && term.grid().history_size() > WINDOW {
            self.take(term, 0);
        }
    }

    /// Move alacritty's oldest rows here until it keeps `keep`, then put its limit back. Rows the
    /// cap would drop at once aren't copied.
    fn take(&mut self, term: &mut Term<Listener>, keep: usize) {
        let limit = self.limit();
        let g = term.grid_mut();
        let h = g.history_size();
        if h > keep {
            let room = self.cap - keep;
            if h - keep >= room {
                self.clear();
            }
            let skip = (h - keep).saturating_sub(room);
            for n in (keep + 1..=h - skip).rev() {
                self.push(&g[Line(-(n as i32))]);
            }
            g.update_history(keep);
        }
        g.update_history(limit);
    }

    /// After a piece: no more than `cap` lines kept, the store's oldest going first.
    pub(super) fn trim(&mut self, term: &mut Term<Listener>) {
        let shown = if alt(term) { 0 } else { term.grid().history_size() };
        let keep = self.cap.saturating_sub(shown);
        if self.rows.len() > keep {
            self.rows.drain(..self.rows.len() - keep);
            // a lowered cap: give back what the rows took
            if self.rows.capacity() > self.rows.len() + 2 * GROW {
                self.rows.shrink_to(self.rows.len() + GROW);
            }
        }
        if shown > self.cap {
            let g = term.grid_mut();
            g.update_history(self.cap);
            g.update_history(self.limit());
        }
    }

    /// Resize `term`. alacritty re-wraps its own rows, within the cap like any push; stored rows
    /// aren't re-wrapped.
    pub(super) fn resize(&mut self, term: &mut Term<Listener>, size: impl Dimensions) {
        term.resize(size);
        self.settle(term);
        self.trim(term);
    }
}
