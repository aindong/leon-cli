//! What a client reads of a terminal beyond its screen: absolute line numbers, history ranges,
//! find, selections, links, and mouse reports (docs/architecture/protocol-and-clients.md,
//! "Scrollback in the app").

use alacritty_terminal::grid::Scroll;
use alacritty_terminal::index::{Column, Line};

use super::*;
use crate::mouse;
use crate::screen::{self, Frame, Lines, Row};
use crate::search::{self, Found, Kind, Link, Pos, Query, Selected};
use crate::theme::Theme;

/// Output is counted this many bytes at a time at most (`piece`), and finding the newest history
/// row again takes at most a piece's pushes.
const SLICE: usize = 4096;
/// What the count moves by when a piece's pushes can't be known: past anything a client keeps,
/// so no old number lands on a new line.
const GAP: usize = MAX_SCROLLBACK + MAX_SIZE.0 as usize;
/// The most rows one `lines` call returns.
pub const MAX_LINES: usize = 1000;

/// Feed output to the emulator, adding the lines it pushed into the scrollback to `scrolled`.
/// The store (terminal_history.rs) only drops what the cap no longer keeps; rows move into it
/// when output pauses (`Store::settle`).
pub(super) fn feed(emu: &mut Emu, parser: &mut Processor, escapes: &mut Escapes, bytes: &[u8], scrolled: &AtomicUsize) {
    let mut rest = bytes;
    while !rest.is_empty() {
        let Emu { term, store } = &mut *emu;
        // a slice ends at a history clear, so the screen it cleared is the one showing after it
        let len = rest.len().min(SLICE);
        let (len, clears) = match escapes.feed_to_clear(&rest[..len]) {
            Some(end) => (end, true),
            None => (len, false),
        };
        let (slice, more) = rest.split_at(len);
        rest = more;
        let guess = memchr::memchr_iter(b'\n', slice).count();
        let mut pushed = counting(term, store.limit(), guess, |t| parser.advance(t, slice));
        // `ESC c` resets both screens; `CSI 3 J` clears only the one showing
        if escapes.take_reset() || clears && !term.mode().contains(TermMode::ALT_SCREEN) {
            store.clear();
        }
        store.trim(term);
        let history = emu.history();
        if clears {
            // a history clear with nothing pinned can't be seen afterwards
            pushed = GAP + history;
        }
        // never below the history, so the oldest kept line has a number
        let n = (scrolled.load(Ordering::SeqCst) + pushed).max(history);
        scrolled.store(n, Ordering::SeqCst);
    }
}

/// Follows, across reads, the escapes leon acts on that alacritty doesn't report: `CSI ? Pm h` /
/// `CSI ? Pm l` for mode 9 (X10 mouse), which it doesn't parse, and the history clears (`CSI 3 J`
/// with any leading zeros or more parameters after the 3, and `ESC c`). Text is skipped to the next ESC with a vectorised search, so most of the output is
/// looked at once. A sequence's parameters are kept up to `MAX_PARAMS` bytes; a longer one is
/// dropped, so a program can't make it buffer without bound.
#[derive(Debug, Default)]
pub struct Escapes {
    x10: bool,
    /// An `ESC c` was seen since `take_reset`.
    reset: bool,
    state: Scan,
    params: Vec<u8>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
enum Scan {
    #[default]
    Text,
    Esc,
    Csi,
    /// A `CSI` with parameters: a `J` next clears the history when the first is 3.
    Ansi,
    Params,
}

const MAX_PARAMS: usize = 64;

impl Escapes {
    /// Whether the program has X10 mouse reporting (`?9`) on.
    pub fn x10(&self) -> bool {
        self.x10
    }

    /// Whether an `ESC c` (a reset of both screens) was seen since the last call.
    pub fn take_reset(&mut self) -> bool {
        std::mem::take(&mut self.reset)
    }

    /// Follow `bytes`; true when a history clear ended in them.
    pub fn feed(&mut self, bytes: &[u8]) -> bool {
        let (mut cleared, mut rest) = (false, bytes);
        while let Some(end) = self.feed_to_clear(rest) {
            (cleared, rest) = (true, &rest[end..]);
        }
        cleared
    }

    /// Follow `bytes` up to the end of the first history clear in them, and say where that is;
    /// None when there is none (all of `bytes` was followed).
    pub fn feed_to_clear(&mut self, bytes: &[u8]) -> Option<usize> {
        let mut i = 0;
        while i < bytes.len() {
            // only an ESC leaves the text state; `ESC x` and `ESC [ x` that start nothing
            // followed here go back to it, so they are skipped as `step` would
            if self.state == Scan::Text {
                loop {
                    let n = memchr::memchr(0x1b, &bytes[i..])?;
                    i += n;
                    match (bytes.get(i + 1), bytes.get(i + 2)) {
                        (Some(b'['), Some(x)) if !matches!(x, b'?' | b'0'..=b'9' | 0x1b) => i += 3,
                        (Some(b), _) if !matches!(b, b'[' | b'c' | 0x1b) => i += 2,
                        _ => break,
                    }
                }
            }
            i += 1;
            if self.step(bytes[i - 1]) {
                return Some(i);
            }
        }
        None
    }

    fn step(&mut self, b: u8) -> bool {
        let mut cleared = false;
        self.state = match (self.state, b) {
            (_, 0x1b) => Scan::Esc,
            (Scan::Esc, b'[') => Scan::Csi,
            (Scan::Esc, b'c') => {
                (self.x10, cleared, self.reset) = (false, true, true); // RIS
                Scan::Text
            }
            (Scan::Csi, b'0'..=b'9' | b';' | b':') => {
                self.params.clear();
                self.params.push(b);
                Scan::Ansi
            }
            (Scan::Ansi, b'0'..=b'9' | b';' | b':') if self.params.len() < MAX_PARAMS => {
                self.params.push(b);
                Scan::Ansi
            }
            (Scan::Ansi, b'J') => {
                let first = self.params.split(|&c| c == b';' || c == b':').next().unwrap_or_default();
                cleared = first.iter().skip_while(|&&c| c == b'0').eq(b"3");
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
fn counting(term: &mut Term<Listener>, limit: usize, guess: usize, f: impl FnOnce(&mut Term<Listener>)) -> usize {
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
    let (overflowed, after) = (g.display_offset() >= limit, g.history_size());
    g.scroll_display(Scroll::Bottom);
    match anchor {
        _ if !primary || overflowed => GAP + after, // a switch hid what was pushed, or it overflowed
        None if after < limit => after,             // filled from empty
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
        let (shot, scrolled, history, x10) = {
            let emu = lock(&self.term);
            (screen::Shot::of(&emu.term), self.scrolled(), emu.history(), self.x10.load(Ordering::SeqCst))
        };
        let mut f = screen::build(&shot, theme);
        (f.view.scrolled, f.view.history) = (scrolled, history);
        f.view.mouse = mouse::report(shot.mode, x10);
        f
    }

    /// Rows `from..from + count` by absolute number (at most `MAX_LINES`), history and screen;
    /// lines not kept are left out.
    pub fn lines(&self, from: usize, count: usize, theme: &Theme) -> Lines {
        let emu = lock(&self.term);
        let (scrolled, history, stored) = (self.scrolled(), emu.history(), emu.stored());
        let end = from.saturating_add(count.min(MAX_LINES));
        // the store's rows come first, then alacritty's history and screen
        let first = scrolled.saturating_sub(history);
        let mut rows: Vec<Row> = (from.max(first)..end.min(first + stored))
            .map(|line| {
                let cells = emu.store.cells(line - first);
                let cols = emu.columns();
                let (mut spans, backgrounds) = screen::row_spans(cells.iter().take(cols), emu.colors(), theme);
                // a stored row isn't re-wrapped: what a narrower pane can't show is cut
                spans.retain(|s| s.col + s.cells <= cols);
                Row { line, spans, backgrounds }
            })
            .collect();
        let from = from.max(first + stored);
        let start = (from as i128 - scrolled as i128).clamp(i32::MIN as i128, i32::MAX as i128) as i32;
        let grid = screen::grid_rows(&emu.term, theme, start, end.saturating_sub(from));
        rows.extend(grid.into_iter().map(|(l, spans, backgrounds)| Row {
            line: (scrolled as i64 + l as i64) as usize,
            spans,
            backgrounds,
        }));
        Lines { scrolled, history, rows }
    }

    pub fn find(&self, q: &Query) -> Result<Found, String> {
        self.with_term(|term| search::find(term, self.scrolled(), q))
    }

    pub fn selection(&self, kind: Kind, anchor: Pos, head: Pos, text: bool) -> Option<Selected> {
        search::select(&mut lock(&self.term).term, self.scrolled(), kind, anchor, head, text)
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
