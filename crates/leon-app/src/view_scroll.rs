//! Scrolling a pane back through its history (docs/architecture/protocol-and-clients.md,
//! "Scrollback in the app"): per-pane view state, history rows fetched from the server in ranges,
//! the wheel and trackpad, paging keys, and the "↓ N new lines" pill.

use std::collections::{HashMap, HashSet};

use gpui::{ScrollWheelEvent, TouchPhase};
use leon_core::screen::{Lines, Row};
use leon_core::search::{Kind, Link, Pos, Range};

use super::calls::{Reply, Waiting};
use super::*;
use crate::scrollback::{self, Composed};

/// History rows fetched per call, around what the view needs.
const FETCH: usize = 200;
/// Cached history rows per pane; past this the cache starts over around the view.
const MAX_CACHED: usize = 5000;

/// Where each pane's grid was painted, by block.
pub(super) type PaintedMap = Rc<RefCell<HashMap<String, Painted>>>;
/// Opens a link.
pub(super) type Opener = Rc<dyn Fn(&str)>;

/// The M11 state of the panes: scroll, selection, find, links.
#[derive(Default)]
pub(super) struct TermUi {
    pub(super) panes: HashMap<String, PaneView>,
    /// Where each pane's grid was painted last, for hit tests.
    pub(super) painted: PaintedMap,
    pub(super) find: Option<find::Find>,
    /// The link under the mouse while cmd is held.
    pub(super) hovered: Option<(String, Link)>,
    /// The mouse's last position in the window.
    pub(super) mouse_at: Option<gpui::Point<Pixels>>,
    /// Opens a link; None is the system handler (tests put a recorder here).
    pub(super) opener: Option<Opener>,
    /// Copy mode and hints, on the pane they started on (`view_copy.rs`).
    pub(super) copy: Option<crate::copy_mode::CopyMode>,
    pub(super) hints: Option<super::copy::Hints>,
    /// Replies that came over a link, waiting for the poll (`term_call_then`), and what wakes it.
    pub(super) replies: Arc<Mutex<Vec<Reply>>>,
    pub(super) wake: Option<Arc<dyn Fn() + Send + Sync>>,
    /// Over a link, the kinds of call with one out, and the newest call of each kind waiting for it
    /// (`term_call_latest`).
    pub(super) in_flight: HashSet<String>,
    pub(super) waiting: HashMap<String, Waiting>,
    /// Text a copy over a link built, for the poll to put on the clipboard, and how many copies
    /// started (a newer one drops an older one still fetching).
    pub(super) clip: Option<String>,
    pub(super) copies: u64,
    /// Over a link, the cell cmd+hover last asked about (a late answer for another is dropped).
    pub(super) hover_at: Option<(String, Pos)>,
    /// Numbers each history fetch, so a reply only fills the pane that asked.
    pub(super) fetches: u64,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Painted {
    pub(super) origin: gpui::Point<Pixels>,
    pub(super) cell: (Pixels, Pixels),
    pub(super) size: (usize, usize),
}

#[derive(Default)]
pub(super) struct PaneView {
    /// The absolute line on the top row while scrolled back; None follows the output.
    pub(super) top: Option<usize>,
    /// Trackpad movement short of a whole line, in lines; momentum events add to it too.
    carry: f32,
    /// `scrolled` when the view left the bottom: the pill counts from there.
    left_at: usize,
    pub(super) cache: HashMap<usize, Row>,
    /// What the cache and every number belong to: (lines, cols, alt screen).
    key: (usize, usize, bool),
    history: usize,
    pub(super) selection: Option<Sel>,
    /// The button held down and reported to the program, if any.
    pub(super) reporting: Option<leon_core::mouse::Button>,
    /// The history fetch out for this pane, if any: one at a time, and only its reply lands.
    fetching: Option<u64>,
}

pub(super) struct Sel {
    pub(super) kind: Kind,
    pub(super) anchor: Pos,
    pub(super) head: Pos,
    /// What the server made of it; None until the drag covers a cell.
    pub(super) range: Option<Range>,
    pub(super) dragging: bool,
}

impl Workspace {
    /// The screen the server last pushed for `id` (empty before the first).
    pub(super) fn live(&self, id: &str) -> Arc<Frame> {
        self.app.model().screens.get(id).cloned().unwrap_or_default()
    }

    /// The session block `id` is in.
    pub(super) fn session_of(&self, id: &str) -> String {
        self.app.model().block(id).map(|(s, _)| s.id.clone()).unwrap_or_default()
    }

    /// Before a pane is drawn: a new size or screen starts its numbering over (the cache,
    /// selection and scroll go, copy mode too if its lines went, and hints label the view again), a history clear drops
    /// what's gone, and missing rows are fetched.
    pub(super) fn sync_pane(&mut self, id: &str) {
        let live = self.live(id);
        let pinned = self.term.copy.as_ref().is_some_and(|c| c.block == id);
        let key = (live.lines, live.cols, live.view.alt_screen);
        let pv = self.term.panes.entry(id.to_string()).or_default();
        let renumbered = pv.key != key || live.view.history < pv.history;
        if renumbered {
            *pv = PaneView { key, ..PaneView::default() };
        }
        pv.history = live.view.history;
        let (oldest, _) = scrollback::bounds(&live);
        if pv.selection.as_ref().and_then(|s| s.range).is_some_and(|r| r.start.line < oldest) {
            pv.selection = None;
        }
        let top = scrollback::clamp(&live, pv.top, pinned);
        if pv.top.is_none() && top.is_some() {
            pv.left_at = live.view.scrolled;
        }
        pv.top = top;
        let missing = scrollback::compose(&live, pv.top, &pv.cache).missing;
        self.check_copy(id);
        if renumbered {
            // hints label the new view again
            if self.term.hints.take_if(|h| h.block == id).is_some() {
                self.open_hints(id.to_string());
            }
        }
        if let Some(&first) = missing.first() {
            self.fetch(id, first.saturating_sub(FETCH / 2).max(oldest), live.lines + FETCH, first);
        }
    }

    /// History rows `from` on of pane `id`, in this app's colours (`Remote::lines`), waited for.
    pub fn lines(&self, id: &str, from: usize, count: usize) -> Option<Lines> {
        let got = self.app.lines(&self.session_of(id), id, from, count);
        got.ok().and_then(|v| serde_json::from_value::<Lines>(v).ok())
    }

    /// Fetch `count` history rows of pane `id` from `from` into its cache, one fetch per pane at a
    /// time (over a link the rows come on a later poll). A reply trimmed to its byte cap can end
    /// before `need`: then it asks from there, or that row never comes.
    fn fetch(&mut self, id: &str, from: usize, count: usize, need: usize) {
        self.term.fetches += 1;
        let ticket = self.term.fetches;
        let pv = self.term.panes.entry(id.to_string()).or_default();
        if pv.fetching.is_some() {
            return;
        }
        pv.fetching = Some(ticket);
        let block = id.to_string();
        self.term_call_then(id, "lines", self.app.lines_args(from, count), move |ws, got| {
            if let Err(e) = &got {
                ws.app.lines_failed(e); // a server too old for the palette is asked without it next time
            }
            // a pane renumbered meanwhile (a resize, a history clear) dropped this fetch
            let Some(pv) = ws.term.panes.get_mut(&block).filter(|p| p.fetching == Some(ticket)) else { return };
            pv.fetching = None;
            let Some(lines) = got.ok().and_then(|v| serde_json::from_value::<Lines>(v).ok()) else { return };
            if pv.cache.len() > MAX_CACHED {
                pv.cache.clear();
            }
            let scrolled = lines.scrolled;
            pv.cache.extend(lines.rows.into_iter().filter(|r| r.line < scrolled).map(|r| (r.line, r)));
            if from < need && !pv.cache.contains_key(&need) {
                ws.fetch(&block, need, count, need);
            }
        });
    }

    /// History line `line` of pane `id`, if cached.
    pub(super) fn cached_row(&self, id: &str, line: usize) -> Option<&Row> {
        self.term.panes.get(id)?.cache.get(&line)
    }

    /// History line `line` of pane `id`, fetched with the rows around it when it isn't cached (over
    /// a link the fetch only starts: the row is there next time).
    pub(super) fn history_row(&mut self, id: &str, line: usize) -> Option<Row> {
        if let Some(r) = self.cached_row(id, line) {
            return Some(r.clone());
        }
        let (oldest, _) = scrollback::bounds(&self.live(id));
        self.fetch(id, line.saturating_sub(FETCH / 2).max(oldest), FETCH, line);
        self.cached_row(id, line).cloned()
    }

    /// What pane `id` shows now.
    pub(super) fn composed(&self, id: &str) -> Composed {
        let empty = HashMap::new();
        let pv = self.term.panes.get(id);
        scrollback::compose(&self.live(id), pv.and_then(|p| p.top), pv.map_or(&empty, |p| &p.cache))
    }

    /// Scroll pane `id` by `lines` (up into the history when positive).
    pub(super) fn scroll_by(&mut self, id: &str, lines: i64) {
        let live = self.live(id);
        let now = self.term.panes.get(id).and_then(|p| p.top).unwrap_or(live.view.scrolled);
        let to = (now as i64 - lines).max(0) as usize;
        self.scroll_to(id, Some(to));
    }

    /// Show `top` on pane `id`'s top row (None, or the bottom, follows the output again).
    pub(super) fn scroll_to(&mut self, id: &str, top: Option<usize>) {
        let live = self.live(id);
        let pv = self.term.panes.entry(id.to_string()).or_default();
        let top = scrollback::clamp(&live, top, false);
        if pv.top.is_none() && top.is_some() {
            pv.left_at = live.view.scrolled;
        }
        pv.top = top;
        self.sync_pane(id);
    }

    /// The wheel or trackpad over pane `id`: reported to a program that asked for the mouse (or
    /// as arrows on the alternate screen), else the view scrolls. Pixel deltas add up across
    /// events, momentum ones too, so slow and gliding moves both scroll.
    pub(super) fn wheel(&mut self, id: &str, ev: &ScrollWheelEvent, cx: &mut Context<Self>) {
        let Some(p) = self.term.painted.borrow().get(id).copied() else { return };
        let dy = f32::from(ev.delta.pixel_delta(p.cell.1).y) / f32::from(p.cell.1).max(1.);
        let pv = self.term.panes.entry(id.to_string()).or_default();
        pv.carry += dy;
        // a hair of slack, so ten 0.3s make three lines despite float rounding
        let lines = ((pv.carry.abs() + 1e-3).trunc() * pv.carry.signum()) as i64;
        pv.carry -= lines as f32;
        if matches!(ev.touch_phase, TouchPhase::Ended) && lines == 0 {
            pv.carry = 0.;
        }
        if lines == 0 {
            return;
        }
        let live = self.live(id);
        let v = live.view;
        let to_program = (v.mouse.is_some() && !ev.modifiers.shift) || (v.alt_screen && v.alternate_scroll);
        if to_program {
            let (line, col) = self.cell_at(id, ev.position).unwrap_or_default();
            let at_bottom = self.term.panes.get(id).is_none_or(|p| p.top.is_none());
            if at_bottom {
                let mods = mods(&ev.modifiers);
                let event = json!({ "type": "wheel", "lines": lines, "line": line, "col": col, "mods": mods });
                self.block_event(id, event);
                return;
            }
        }
        if !v.alt_screen {
            self.scroll_by(id, lines);
            cx.notify();
        }
    }

    /// The view cell (row, col) under `at` in pane `id`, clamped to the grid.
    pub(super) fn cell_at(&self, id: &str, at: gpui::Point<Pixels>) -> Option<(usize, usize)> {
        let p = self.term.painted.borrow().get(id).copied()?;
        let x = f32::from(at.x - p.origin.x) / f32::from(p.cell.0);
        let y = f32::from(at.y - p.origin.y) / f32::from(p.cell.1);
        let clamp = |v: f32, n: usize| (v.max(0.) as usize).min(n.saturating_sub(1));
        Some((clamp(y, p.size.0), clamp(x, p.size.1)))
    }

    /// The scroll actions, on the target pane.
    pub(super) fn scroll_action(&mut self, name: &str, cx: &mut Context<Self>) {
        let Some((_, id, _)) = self.target() else { return };
        let live = self.live(&id);
        let page = live.lines.saturating_sub(1).max(1) as i64;
        match name {
            "client.scroll.page_up" => self.scroll_by(&id, page),
            "client.scroll.page_down" => self.scroll_by(&id, -page),
            "client.scroll.top" => self.scroll_to(&id, Some(scrollback::bounds(&live).0)),
            _ => self.scroll_to(&id, None),
        }
        cx.notify();
    }

    /// The "↓ N new lines" pill on a pane scrolled back while output arrived; a click goes back
    /// to the bottom.
    pub(super) fn render_jump(&self, id: &str, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let n = self.new_lines(id)?;
        let t = &self.pane_theme(id); // faded with its pane
        let block = id.to_string();
        let label = if n == 1 { "↓ 1 new line".to_string() } else { format!("↓ {n} new lines") };
        Some(
            div()
                .id(SharedString::from(format!("jump-{id}")))
                .debug_selector(|| sel(format!("jump-{id}")))
                .absolute()
                .bottom(px(10.))
                .right(px(14.))
                .px(px(10.))
                .py(px(3.))
                .rounded(px(10.))
                .bg(rgb(t.hover))
                .border_1()
                .border_color(rgb(t.border))
                .text_size(px(SECONDARY_TEXT))
                .text_color(rgb(t.text))
                .cursor_pointer()
                .hover(|d| d.text_color(rgb(t.title)))
                .child(label)
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |ws, _, _, cx| {
                        cx.stop_propagation();
                        ws.scroll_to(&block, None);
                        cx.notify();
                    }),
                ),
        )
    }

    /// The text pane `id` shows now, a line per row (scrolled back or not).
    pub fn view_text(&self, id: &str) -> Option<String> {
        let shown = self.app.model().screens.contains_key(id);
        shown.then(|| self.composed(id).frame.text())
    }

    /// How many history rows pane `id` holds; None when the workspace keeps nothing for it.
    pub fn cached_rows(&self, id: &str) -> Option<usize> {
        self.term.panes.get(id).map(|p| p.cache.len())
    }

    /// The absolute line on pane `id`'s top row while scrolled back; None when following output.
    pub fn view_top(&self, id: &str) -> Option<usize> {
        self.term.panes.get(id).and_then(|p| p.top)
    }

    /// How many lines arrived since pane `id` was scrolled back, when the pill shows them.
    pub fn new_lines(&self, id: &str) -> Option<usize> {
        let pv = self.term.panes.get(id).filter(|p| p.top.is_some())?;
        let n = self.live(id).view.scrolled.saturating_sub(pv.left_at);
        (n > 0).then_some(n)
    }

    /// The middle of cell (row, col) of pane `id`'s view, in window coordinates.
    pub fn cell_position(&self, id: &str, row: usize, col: usize) -> Option<gpui::Point<Pixels>> {
        let p = self.term.painted.borrow().get(id).copied()?;
        let (w, h) = p.cell;
        Some(point(p.origin.x + w * (col as f32 + 0.5), p.origin.y + h * (row as f32 + 0.5)))
    }
}

/// GPUI's modifiers as a mouse report's.
pub(super) fn mods(m: &gpui::Modifiers) -> Value {
    json!({ "shift": m.shift, "alt": m.alt, "ctrl": m.control })
}
