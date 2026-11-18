//! The scrollback past alacritty's window (docs/architecture/protocol-and-clients.md, "Scrollback
//! in the app"). alacritty keeps every history row at the pane's full width, 24 bytes a cell; so
//! it keeps only the newest `WINDOW` rows, and older ones move here as their text and runs of
//! style, about what the text costs. The store's rows are numbered just above alacritty's oldest,
//! so a line keeps its number as it moves.

use std::collections::VecDeque;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use alacritty_terminal::grid::{Dimensions, GridCell, Row};
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::{Cell, CellExtra, Flags, LineLength};
use alacritty_terminal::term::{Term, TermMode};
use alacritty_terminal::vte::ansi::Color;

use super::{lock, Listener, Terminal, MAX_SCROLLBACK};

/// The history rows alacritty keeps: its storage grows once to the screen plus these and no further
/// (it allocates history 1,000 rows at a time).
pub const WINDOW: usize = 1000;
/// Room for this many pushes is kept in alacritty's window for a return from the alternate
/// screen (`feed`).
pub(super) const PUSHES: usize = 100;
/// The rows alacritty keeps before a piece: room for a piece's pushes, twice over (a piece that
/// starts on the alternate screen pushes on the primary one only after switching back).
const KEEP: usize = WINDOW - 2 * PUSHES;
/// Stored rows are allocated this many at a time.
const GROW: usize = 1024;
/// alacritty's limit while it re-wraps for a resize, so the re-wrap drops nothing it would push
/// past `WINDOW`: a row of the widest grid re-wraps to at most this many rows of the narrowest.
const REFLOW: usize = WINDOW * super::MAX_SIZE.1 as usize;

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
            store.make_room(term);
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
/// colour or a line), its runs, and the few cells with combining marks, an underline colour or an
/// OSC 8 link (alacritty's `CellExtra`, shared with the cells it came from).
struct Stored {
    text: Box<str>,
    runs: Box<[Run]>,
    extras: Box<[(u16, Arc<CellExtra>)]>,
}

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
        // alacritty's text ends at the last character; a coloured blank after it still draws
        let drawn = row.into_iter().rposition(|c| !c.is_empty()).map_or(0, |i| i + 1);
        let len = row.line_length().0.max(drawn);
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
        self.rows.push_back(Stored { text: text.into(), runs: runs.into(), extras: extras.into() });
    }

    /// Stored row `i`'s cells as alacritty had them, trailing blanks left out.
    pub(super) fn cells(&self, i: usize) -> Vec<Cell> {
        let Some(row) = self.rows.get(i) else { return Vec::new() };
        let mut chars = row.text.chars();
        let mut out = Vec::with_capacity(row.text.len());
        for r in row.runs.iter() {
            for c in chars.by_ref().take(r.cells.into()) {
                out.push(Cell { c, fg: r.fg, bg: r.bg, flags: r.flags, extra: None });
            }
        }
        for (col, e) in row.extras.iter() {
            out[usize::from(*col)].extra = Some(e.clone());
        }
        out
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

    /// Before a piece of output on the primary screen: move alacritty's oldest rows past `KEEP`
    /// here, so the piece's pushes fit in its `WINDOW` and none is dropped uncopied.
    pub(super) fn make_room(&mut self, term: &mut Term<Listener>) {
        if !alt(term) {
            let keep = KEEP.min(self.cap);
            self.take(term, keep);
        }
    }

    /// Move alacritty's oldest rows here until it keeps `keep`, then put its limit back.
    fn take(&mut self, term: &mut Term<Listener>, keep: usize) {
        let g = term.grid_mut();
        let h = g.history_size();
        if h > keep {
            for n in (keep + 1..=h).rev() {
                self.push(&g[Line(-(n as i32))]);
            }
            g.update_history(keep);
        }
        g.update_history(WINDOW);
    }

    /// After a piece: no more than `cap` lines kept, the store's oldest going first.
    pub(super) fn trim(&mut self, term: &mut Term<Listener>) {
        let shown = if alt(term) { 0 } else { term.grid().history_size() };
        let keep = self.cap.saturating_sub(shown);
        if self.rows.len() > keep {
            self.rows.drain(..self.rows.len() - keep);
        }
        if shown > self.cap {
            let g = term.grid_mut();
            g.update_history(self.cap);
            g.update_history(WINDOW);
        }
    }

    /// Resize `term`, moving into the store what its re-wrap pushes past its window instead of
    /// letting alacritty drop it. Stored rows aren't re-wrapped. On the alternate screen the
    /// primary grid is out of reach (alacritty's inactive grid), so its re-wrap keeps `WINDOW`.
    pub(super) fn resize(&mut self, term: &mut Term<Listener>, size: impl Dimensions) {
        let primary = !alt(term);
        if primary {
            term.grid_mut().update_history(REFLOW);
        }
        term.resize(size);
        if primary {
            self.make_room(term);
        }
        self.trim(term);
    }
}
