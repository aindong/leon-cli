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
pub(super) fn feed(
    term: &mut Term<Listener>,
    parser: &mut Processor,
    escapes: &mut Escapes,
    bytes: &[u8],
    scrolled: &AtomicUsize,
) {
    for slice in bytes.chunks(SLICE) {
        // a history clear with nothing pinned can't be seen afterwards
        let clears = escapes.feed(slice);
        let guess = memchr::memchr_iter(b'\n', slice).count();
        let pushed = counting(term, guess, |t| parser.advance(t, slice));
        let pushed = if clears { GAP + term.grid().history_size() } else { pushed };
        // never below the history, so the oldest kept line has a number
        let n = (scrolled.load(Ordering::SeqCst) + pushed).max(term.grid().history_size());
        scrolled.store(n, Ordering::SeqCst);
    }
}

/// Follows, across reads, the escapes leon acts on that alacritty doesn't report: `CSI ? Pm h` /
/// `CSI ? Pm l` for mode 9 (X10 mouse), which it doesn't parse, and the history clears (`CSI 3 J`,
/// `ESC c`). Text is skipped to the next ESC with a vectorised search, so most of the output is
/// looked at once. A sequence's parameters are kept up to `MAX_PARAMS` bytes; a longer one is
/// dropped, so a program can't make it buffer without bound.
#[derive(Debug, Default)]
pub struct Escapes {
    x10: bool,
    state: Scan,
    params: Vec<u8>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
enum Scan {
    #[default]
    Text,
    Esc,
    Csi,
    /// `CSI 3`: a `J` next clears the history.
    Three,
    Params,
}

const MAX_PARAMS: usize = 64;

impl Escapes {
    /// Whether the program has X10 mouse reporting (`?9`) on.
    pub fn x10(&self) -> bool {
        self.x10
    }

    /// Follow `bytes`; true when a history clear ended in them.
    pub fn feed(&mut self, bytes: &[u8]) -> bool {
        let (mut cleared, mut i) = (false, 0);
        while i < bytes.len() {
            // only an ESC leaves the text state; `ESC x` and `ESC [ x` that start nothing
            // followed here go back to it, so they are skipped as `step` would
            if self.state == Scan::Text {
                loop {
                    let Some(n) = memchr::memchr(0x1b, &bytes[i..]) else { return cleared };
                    i += n;
                    match (bytes.get(i + 1), bytes.get(i + 2)) {
                        (Some(b'['), Some(x)) if !matches!(x, b'?' | b'3' | 0x1b) => i += 3,
                        (Some(b), _) if !matches!(b, b'[' | b'c' | 0x1b) => i += 2,
                        _ => break,
                    }
                }
            }
            cleared |= self.step(bytes[i]);
            i += 1;
        }
        cleared
    }

    fn step(&mut self, b: u8) -> bool {
        let mut cleared = false;
        self.state = match (self.state, b) {
            (_, 0x1b) => Scan::Esc,
            (Scan::Esc, b'[') => Scan::Csi,
            (Scan::Esc, b'c') => {
                (self.x10, cleared) = (false, true); // RIS
                Scan::Text
            }
            (Scan::Csi, b'3') => Scan::Three,
            (Scan::Three, b'J') => {
                cleared = true;
                Scan::Text
            }
            (Scan::Csi, b'?') => {
                self.params.clear();
                Scan::Params
            }
            (Scan::Params, b'0'..=b'9' | b';') if self.params.len() < MAX_PARAMS => {
                self.params.push(b);
                Scan::Params
            }
            (Scan::Params, b'h' | b'l') => {
                // the mouse modes replace each other (xterm): setting or resetting another ends X10
                for p in self.params.split(|&c| c == b';') {
                    match p {
                        b"9" => self.x10 = b == b'h',
                        b"1000" | b"1001" | b"1002" | b"1003" => self.x10 = false,
                        _ => {}
                    }
                }
                Scan::Text
            }
            _ => Scan::Text,
        };
        cleared
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
fn counting(term: &mut Term<Listener>, guess: usize, f: impl FnOnce(&mut Term<Listener>)) -> usize {
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
            let at = |n: i32| std::ptr::eq(&g[Line(-n)][Column(0)], a);
            // a line feed per pushed line is the usual case: look there first
            let first = (guess < after).then_some(guess as i32 + 1).filter(|&n| at(n));
            first.or_else(|| (1..=after as i32).find(|&n| at(n))).map_or(GAP + after, |n| n as usize - 1)
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
        // only the copy under the lock: the parse thread waits for that, not for the spans
        let (shot, scrolled, x10) =
            self.with_term(|term| (screen::Shot::of(term), self.scrolled(), self.x10.load(Ordering::SeqCst)));
        let mut f = screen::build(&shot, theme);
        f.view.scrolled = scrolled;
        f.view.mouse = mouse::report(shot.mode, x10);
        f
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

#[cfg(test)]
#[path = "terminal_view_tests.rs"]
mod tests;
