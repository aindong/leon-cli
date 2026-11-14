//! What a client reads of a terminal beyond its screen: absolute line numbers, history ranges,
//! find, selections, links, and mouse reports (docs/architecture.md, "Scrollback in the app").

use alacritty_terminal::grid::Scroll;
use alacritty_terminal::index::{Column, Line};

use super::*;
use crate::mouse;
use crate::screen::{self, Frame, Lines, Row};
use crate::search::{self, Found, Kind, Link, Pos, Query, Selected};
use crate::theme::Theme;

/// Output is counted this many bytes at a time: a byte pushes at most one line into the
/// scrollback (short of `CSI n S`), so a slice rarely pushes the whole scrollback's worth, and
/// finding the newest history row again takes at most a slice's pushes.
const SLICE: usize = 4096;
/// What the count moves by when a slice's pushes can't be known: past anything a client keeps,
/// so no old number lands on a new line.
const GAP: usize = SCROLLBACK + MAX_SIZE.0 as usize;
/// The most rows one `lines` call returns.
pub const MAX_LINES: usize = 1000;

/// Feed output to the emulator, adding the lines it pushed into the scrollback to `scrolled`.
pub(super) fn feed(term: &mut Term<Listener>, parser: &mut Processor, bytes: &[u8], scrolled: &AtomicUsize) {
    for slice in bytes.chunks(SLICE) {
        // a history clear (`CSI 3 J`, `ESC c`) with nothing pinned can't be seen afterwards
        let clears = [b"\x1b[3J".as_slice(), b"\x1bc"].iter().any(|c| slice.windows(c.len()).any(|w| w == *c));
        let pushed = counting(term, |t| parser.advance(t, slice));
        let pushed = if clears { GAP + term.grid().history_size() } else { pushed };
        // never below the history, so the oldest kept line has a number
        let n = (scrolled.load(Ordering::SeqCst) + pushed).max(term.grid().history_size());
        scrolled.store(n, Ordering::SeqCst);
    }
}

/// Run `f` and return how many lines it pushed into the primary screen's scrollback: how far
/// the newest history row moved. Rows are rotated, never copied, so a row's cells stay at one
/// address while it is kept; scrolls that don't reach the scrollback (a delete-line, a top
/// margin) move no history row and count nothing. A row pushed out of the scrollback is reused,
/// so a slice that pushed the whole scrollback's worth could find it again in a new place:
/// alacritty's display offset, set to 1 first, grows with every scroll and caps at the
/// scrollback's size, which says when that may have happened. It is put back to 0 whenever the
/// primary screen is showing, so no frame sees it.
fn counting(term: &mut Term<Listener>, f: impl FnOnce(&mut Term<Listener>)) -> usize {
    let primary = !term.mode().contains(TermMode::ALT_SCREEN);
    let newest = |t: &Term<Listener>| &t.grid()[Line(-1)][Column(0)] as *const _;
    let before = term.grid().history_size();
    let anchor = (primary && before > 0).then(|| newest(term));
    if anchor.is_some() {
        term.grid_mut().scroll_display(Scroll::Delta(1));
    }
    f(term);
    if term.mode().contains(TermMode::ALT_SCREEN) {
        // the primary screen sits untouched under the alternate one (its offset is reset on return)
        return if primary { GAP } else { 0 };
    }
    let g = term.grid_mut();
    let (overflowed, after) = (g.display_offset() >= SCROLLBACK, g.history_size());
    g.scroll_display(Scroll::Bottom);
    match anchor {
        _ if !primary || overflowed => GAP + after, // a switch hid what was pushed, or it overflowed
        None if after < SCROLLBACK => after,        // filled from empty
        None => GAP,
        Some(a) => {
            let g = term.grid();
            (1..=after as i32)
                .find(|&n| std::ptr::eq(&g[Line(-n)][Column(0)], a))
                .map_or(GAP + after, |n| n as usize - 1)
        }
    }
}

impl Terminal {
    /// The absolute number of the screen's top row (`View::scrolled`).
    pub fn scrolled(&self) -> usize {
        self.scrolled.load(Ordering::SeqCst)
    }

    /// The screen a client paints, with its line numbering and mouse modes.
    pub fn frame(&self, theme: &Theme) -> Frame {
        self.with_term(|term| {
            let mut f = screen::frame(term, theme);
            f.view.scrolled = self.scrolled();
            f.view.mouse = mouse::report(*term.mode(), self.x10.load(Ordering::SeqCst));
            f
        })
    }

    /// Rows `from..from + count` by absolute number (at most `MAX_LINES`), history and screen;
    /// lines not kept are left out.
    pub fn lines(&self, from: usize, count: usize, theme: &Theme) -> Lines {
        self.with_term(|term| {
            let scrolled = self.scrolled();
            let start = (from as i128 - scrolled as i128).clamp(i32::MIN as i128, i32::MAX as i128) as i32;
            let rows = screen::grid_rows(term, theme, start, count.min(MAX_LINES));
            let rows = rows.into_iter().map(|(l, spans, backgrounds)| Row {
                line: (scrolled as i64 + l as i64) as usize,
                spans,
                backgrounds,
            });
            Lines { scrolled, history: term.grid().history_size(), rows: rows.collect() }
        })
    }

    pub fn find(&self, q: &Query) -> Result<Found, String> {
        self.with_term(|term| search::find(term, self.scrolled(), q))
    }

    pub fn selection(&self, kind: Kind, anchor: Pos, head: Pos, text: bool) -> Option<Selected> {
        search::select(&mut lock(&self.term), self.scrolled(), kind, anchor, head, text)
    }

    pub fn link_at(&self, at: Pos) -> Option<Link> {
        self.with_term(|term| search::link_at(term, self.scrolled(), at))
    }

    /// Send `e` to the program as its mouse mode asks now; false when the mode doesn't want it.
    pub fn mouse(&self, e: &mouse::Event) -> io::Result<bool> {
        let mode = self.with_term(|t| mouse::Mode::of(*t.mode(), self.x10.load(Ordering::SeqCst)));
        match mouse::encode(e, &mode) {
            Some(bytes) => self.write(&bytes).map(|_| true),
            None => Ok(false),
        }
    }
}
