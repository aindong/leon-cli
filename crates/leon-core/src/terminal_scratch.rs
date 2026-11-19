//! Find, selections and links over the whole history (docs/architecture/scrollback.md). Lines in
//! leon's store are copied, with alacritty's rows next to them where a range runs on into those,
//! into a plain alacritty grid as wide as the pane (a scratch), and alacritty's own search,
//! selection and link code runs on it. A scratch is at most about `CHUNK` rows: a find or a copy
//! of the whole history holds one at a time.

use alacritty_terminal::event::VoidListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line, Point};
use alacritty_terminal::term::cell::{Cell, Flags};
use alacritty_terminal::term::{Config, Term};

use super::history::Emu;
use super::{lock, Terminal};
use crate::program_status::strip_hidden;
use crate::search::{self, Finder, Found, Kind, Link, Pos, Query, Selected};

/// The most rows a scratch holds (a logical line that runs on past it is cut there).
const CHUNK: usize = 1000;

/// A scratch's size.
struct Rows(usize, usize);

impl Dimensions for Rows {
    fn total_lines(&self) -> usize {
        self.0
    }
    fn screen_lines(&self) -> usize {
        self.0
    }
    fn columns(&self) -> usize {
        self.1
    }
}

/// The lines of one terminal by absolute number, its store's and alacritty's alike.
struct View<'a> {
    emu: &'a Emu,
    scrolled: usize,
    /// The oldest line kept.
    first: usize,
    /// The first line alacritty holds; the store's are older.
    boundary: usize,
    /// The screen's last line.
    last: usize,
    cols: usize,
}

impl<'a> View<'a> {
    fn new(emu: &'a Emu, scrolled: usize) -> View<'a> {
        let first = scrolled.saturating_sub(emu.history());
        let (last, cols) = (scrolled + emu.screen_lines() - 1, emu.columns());
        View { emu, scrolled, first, boundary: first + emu.stored(), last, cols }
    }

    /// alacritty's point for line `n`, one it holds.
    fn point(&self, n: usize, col: usize) -> Point {
        Point::new(Line((n as i64 - self.scrolled as i64) as i32), Column(col))
    }

    fn cells(&self, n: usize) -> Vec<Cell> {
        if n < self.boundary {
            return self.emu.store.cells(n - self.first);
        }
        let row = &self.emu.term.grid()[self.point(n, 0).line];
        (0..self.cols).map(|c| row[Column(c)].clone()).collect()
    }

    /// Whether line `n`'s logical line goes on in the next.
    fn wrapped(&self, n: usize) -> bool {
        if n < self.boundary {
            return self.emu.store.wrapped(n - self.first, self.cols);
        }
        n <= self.last && self.emu.term.grid()[self.point(n, self.cols - 1)].flags.contains(Flags::WRAPLINE)
    }

    /// Lines `from..to` in a grid of their own, whose line 0 is `from`. A stored row wider than the
    /// pane is cut at its edge, as `lines` shows it.
    fn scratch(&self, from: usize, to: usize) -> Term<VoidListener> {
        let config = Config { scrolling_history: 0, ..Config::default() };
        let mut t = Term::new(config, &Rows(to - from, self.cols), VoidListener);
        for n in from..to {
            let row = &mut t.grid_mut()[Line((n - from) as i32)];
            for (c, cell) in self.cells(n).into_iter().take(self.cols).enumerate() {
                row[Column(c)] = cell;
            }
        }
        t
    }

    /// Whether alacritty's first row goes on a line that began in the store: then reading it needs
    /// the store too.
    fn crosses(&self) -> bool {
        self.boundary > self.first && self.wrapped(self.boundary - 1)
    }

    /// The lines of the logical line around `n`, at most `most` either way: `(from, to)`.
    fn logical(&self, n: usize, most: usize) -> (usize, usize) {
        let (mut from, mut to) = (n, n);
        while from > self.first && n - from < most && self.wrapped(from - 1) {
            from -= 1;
        }
        while to < self.last && to - n < most && self.wrapped(to) {
            to += 1;
        }
        (from, to + 1)
    }

    /// Where a chunk from `from` ends, at most `CHUNK` rows on and at most `to`: at `to`, else
    /// after a row that ends a logical line, unless a single line runs past the chunk.
    fn cut(&self, from: usize, to: usize) -> usize {
        let most = (from + CHUNK).min(to);
        if most == to {
            return to;
        }
        let mut cut = most;
        while cut > from + 1 && self.wrapped(cut - 1) {
            cut -= 1;
        }
        if cut > from + 1 || !self.wrapped(from) {
            cut
        } else {
            most
        }
    }
}

impl Terminal {
    /// Find in the history and on the screen, oldest match first.
    pub fn find(&self, q: &Query) -> Result<Found, String> {
        let emu = lock(&self.term);
        let v = View::new(&emu, self.scrolled());
        if v.boundary == v.first {
            return search::find(&emu.term, v.scrolled, q);
        }
        let Some(mut f) = Finder::new(q)? else { return Ok(Found::default()) };
        let mut from = v.first;
        while from < v.boundary && f.going() {
            let mut to = v.cut(from, v.boundary);
            // the store's last line may go on in alacritty's rows: searched here, as one line
            while to >= v.boundary && to <= v.last && to - v.boundary < CHUNK && v.wrapped(to - 1) {
                to += 1;
            }
            f.run(&v.scratch(from, to), from, Point::new(Line(0), Column(0)));
            from = to;
        }
        if f.going() && from <= v.last {
            f.run(&emu.term, v.scrolled, v.point(from, 0));
        }
        Ok(f.found)
    }

    /// The selection of `kind` from `anchor` to `head`, in absolute lines (`search::select`).
    pub fn selection(&self, kind: Kind, anchor: Pos, head: Pos, text: bool) -> Option<Selected> {
        let mut emu = lock(&self.term);
        let scrolled = self.scrolled();
        let v = View::new(&emu, scrolled);
        // clamped as `search::select` clamps them
        let anchor = if anchor.line < v.first { Pos { line: v.first, col: 0 } } else { anchor };
        let head = Pos { line: head.line.clamp(v.first, v.last), col: head.col.min(v.cols - 1) };
        if anchor.line.min(head.line) >= v.boundary && !v.crosses() {
            return search::select(&mut emu.term, scrolled, kind, anchor, head, text);
        }
        if anchor.line > v.last || anchor.col >= v.cols {
            return None;
        }
        let (start, end) = ends(&v, kind, anchor, head)?;
        let text = text.then(|| copied(&v, kind, start, end));
        Some(Selected { start, end, text })
    }

    /// The link on the cell at `at` (`search::link_at`).
    pub fn link_at(&self, at: Pos) -> Option<Link> {
        let emu = lock(&self.term);
        let v = View::new(&emu, self.scrolled());
        if at.line >= v.boundary && !v.crosses() {
            return search::link_at(&emu.term, v.scrolled, at);
        }
        if at.line < v.first || at.line > v.last {
            return None;
        }
        // an OSC 8 link runs on over its cells, across line ends too, as far as alacritty looks
        let reach = search::MAX_URI * 4 / v.cols + 2;
        let (from, to) = v.logical(at.line, CHUNK / 2);
        let from = from.min(at.line.saturating_sub(reach)).max(v.first);
        let to = to.max(at.line + reach + 1).min(v.last + 1);
        search::link_at(&v.scratch(from, to), from, at)
    }
}

/// A selection's ends, widened as alacritty widens them, each on a scratch around it.
fn ends(v: &View, kind: Kind, anchor: Pos, head: Pos) -> Option<(Pos, Pos)> {
    let around = |n: usize| v.logical(n, CHUNK / 2);
    if anchor == head {
        // a click: alacritty's whole selection, a bracket's match within a few hundred lines
        let (from, to) = around(anchor.line);
        let (from, to) =
            (from.min(anchor.line.saturating_sub(200)).max(v.first), to.max(anchor.line + 201).min(v.last + 1));
        let s = search::select(&mut v.scratch(from, to), from, kind, anchor, head, false)?;
        return Some((s.start, s.end));
    }
    if kind == Kind::Block {
        let (top, bottom) = (anchor.line.min(head.line), anchor.line.max(head.line));
        let (mut l, mut r) = (anchor.col.min(head.col), anchor.col.max(head.col));
        // half a wide character takes the whole one
        if v.cells(top).get(l).is_some_and(|c| c.flags.contains(Flags::WIDE_CHAR_SPACER)) && l > 0 {
            l -= 1;
        }
        if v.cells(bottom).get(r).is_some_and(|c| c.flags.contains(Flags::WIDE_CHAR)) && r < v.cols - 1 {
            r += 1;
        }
        return Some((Pos { line: top, col: l }, Pos { line: bottom, col: r }));
    }
    let (lo, hi) = (anchor.min(head), anchor.max(head));
    // each end widened on a scratch of its own logical line, as alacritty widens a selection's
    // ends: half a wide character takes the whole one, then words or lines grow outwards
    let widen = |at: Pos, left: bool| {
        let (from, to) = around(at.line);
        let sc = v.scratch(from, to);
        let mut p = Point::new(Line((at.line - from) as i32), Column(at.col));
        let flags = sc.grid()[p].flags;
        if left && flags.contains(Flags::WIDE_CHAR_SPACER) && p.column.0 > 0 {
            p.column -= 1;
        }
        if !left && flags.contains(Flags::WIDE_CHAR) && p.column.0 < v.cols - 1 {
            p.column += 1;
        }
        p = match (kind, left) {
            (Kind::Word, true) => sc.semantic_search_left(p),
            (Kind::Word, false) => sc.semantic_search_right(p),
            (Kind::Line, true) => sc.line_search_left(p),
            (Kind::Line, false) => sc.line_search_right(p),
            _ => p,
        };
        Pos { line: from + p.line.0 as usize, col: p.column.0 }
    };
    let (start, end) = (widen(lo, true), widen(hi, false));
    Some((start, end))
}

/// The text a selection copies, a chunk at a time, as `search::select` gives it.
fn copied(v: &View, kind: Kind, start: Pos, end: Pos) -> String {
    let block = kind == Kind::Block;
    let mut out = String::new();
    let mut from = start.line;
    while from <= end.line {
        let to = v.cut(from, end.line + 1);
        let first = if from == start.line || block { start.col } else { 0 };
        let last = if to == end.line + 1 || block { end.col } else { v.cols - 1 };
        // with the row before, which alacritty reads when a copy's last row wraps a wide character
        let before = usize::from(from > v.first);
        let (top, bottom) = (before as i32, (to - from - 1 + before) as i32);
        let mut scratch = v.scratch(from - before, to);
        let (first, last) = (Point::new(Line(top), Column(first)), Point::new(Line(bottom), Column(last)));
        out += &search::text_of(&mut scratch, block, first, last);
        // a chunk's text ends without the newline alacritty drops at a copy's end
        if to <= end.line && (block || !v.wrapped(to - 1)) {
            out.push('\n');
        }
        from = to;
    }
    if kind == Kind::Line {
        out.push('\n');
    }
    // marks a program stacked on cells can hide characters; the copy holds none it didn't show
    out.split('\n').map(strip_hidden).collect::<Vec<_>>().join("\n")
}
