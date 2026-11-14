//! What a client reads of a terminal beyond its screen: absolute line numbers, history ranges,
//! find, selections, links, and mouse reports (docs/architecture.md, "Scrollback in the app").

use alacritty_terminal::grid::Scroll;

use super::*;
use crate::mouse;
use crate::screen::{self, Frame, Lines, Row};
use crate::search::{self, Found, Kind, Link, Pos, Query, Selected};
use crate::theme::Theme;

/// Output is counted this many bytes at a time: a byte pushes at most one line into the
/// scrollback (short of `CSI n S`), so a slice can't push the whole scrollback's worth.
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

/// Run `f` and return how many lines it pushed into the primary screen's scrollback. While the
/// scrollback isn't full that is its growth. Once it is, the count is alacritty's own (its
/// `clear_viewport` counts this way): a display offset off the bottom grows by every line
/// pushed, so it is set to 1 first and put back to 0 after. Nothing reads it in between.
fn counting(term: &mut Term<Listener>, f: impl FnOnce(&mut Term<Listener>)) -> usize {
    let primary = !term.mode().contains(TermMode::ALT_SCREEN);
    let before = term.grid().history_size();
    let pinned = primary && before > 0;
    if pinned {
        term.grid_mut().scroll_display(Scroll::Delta(1));
    }
    f(term);
    if term.mode().contains(TermMode::ALT_SCREEN) {
        return 0; // the primary grid keeps its offset until it is back; counted then
    }
    let g = term.grid_mut();
    let (offset, after) = (g.display_offset(), g.history_size());
    g.scroll_display(Scroll::Bottom);
    match offset {
        SCROLLBACK => GAP, // the offset hit the cap: more were pushed than it says
        1.. => offset - 1, // pinned (here, or before an alt-screen spell)
        0 if primary && !pinned && after < SCROLLBACK => after - before,
        0 if primary && !pinned => GAP, // filled from empty in one slice: unknown
        _ => GAP + after,               // a clear reset the offset, or the alt screen hid the count
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
            let start = (from as i64 - scrolled as i64).clamp(i32::MIN as i64, i32::MAX as i64) as i32;
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
