//! The mouse on a pane: selections (drag, word, line, shift to extend, option for a rectangle),
//! reports to a program that asked for the mouse (shift selects anyway), cmd+click on a link
//! (view_links.rs); copy and select-all.

use std::collections::HashMap;

use gpui::{ClipboardItem, Modifiers, MouseDownEvent};
use leon_core::mouse::Button;
use leon_core::screen::{Lines, Span};
use leon_core::search::{Kind, Pos, Range, Selected};

use super::scroll::{mods, Sel};
use super::*;
use crate::scrollback;

/// How many times a copy over a link asks the host for rows it doesn't hold (each reply is capped).
const COPY_FETCHES: u32 = 64;

/// A copy over a link on its way: its range, whether it is a block, and which copy it is.
#[derive(Clone, Copy)]
struct Copying {
    range: Range,
    block: bool,
    seq: u64,
}

/// What selection `s` covers, worked out without the server: from anchor to head in reading order
/// (a line selection takes whole lines; a word one, until the server widens it, the cells dragged).
fn local_range(s: &Sel, cols: usize) -> Range {
    let (lo, hi) = (s.anchor.min(s.head), s.anchor.max(s.head));
    match s.kind {
        Kind::Line => {
            Range { start: Pos { line: lo.line, col: 0 }, end: Pos { line: hi.line, col: cols.saturating_sub(1) } }
        }
        _ => Range { start: lo, end: hi },
    }
}

/// Whether `got` only widens `here` on the same lines (a word's ends past the cells dragged).
fn widens(here: Range, got: Range) -> bool {
    got.start.line == here.start.line && got.end.line == here.end.line && got.start <= here.start && got.end >= here.end
}

fn button(b: MouseButton) -> Option<Button> {
    match b {
        MouseButton::Left => Some(Button::Left),
        MouseButton::Middle => Some(Button::Middle),
        MouseButton::Right => Some(Button::Right),
        _ => None,
    }
}

impl Workspace {
    /// The absolute cell under `at` in pane `id`.
    pub(super) fn pos_at(&self, id: &str, at: gpui::Point<Pixels>) -> Option<Pos> {
        let (row, col) = self.cell_at(id, at)?;
        Some(Pos { line: self.composed(id).top + row, col })
    }

    /// Whether a press on pane `id` goes to its program: it asked for the mouse, shift isn't
    /// held, and the view shows the live screen.
    pub(super) fn reports(&self, id: &str, m: &Modifiers) -> bool {
        self.live(id).view.mouse.is_some() && !m.shift && self.view_top(id).is_none()
    }

    /// Send a mouse event to pane `id`'s program at window point `at`.
    fn report(&self, id: &str, action: &str, button: Option<Button>, at: gpui::Point<Pixels>, m: &Modifiers) {
        let Some((line, col)) = self.cell_at(id, at) else { return };
        let event =
            json!({ "type": "mouse", "action": action, "button": button, "line": line, "col": col, "mods": mods(m) });
        self.block_event(id, event);
    }

    pub(super) fn mouse_down(&mut self, id: &str, ev: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if ev.modifiers.platform && ev.button == MouseButton::Left {
            // the link cmd+hover found here; on This Mac, asked if it hasn't, and opened when the
            // answer comes (a host's link only from its hover: a link it names is checked against
            // the text drawn, which a late answer may no longer match)
            let pos = self.pos_at(id, ev.position);
            let hovered = self
                .term
                .hovered
                .clone()
                .filter(|(b, l)| b == id && pos.is_some_and(|p| (l.start..=l.end).contains(&p)));
            let link = match hovered {
                Some((_, l)) => Some(l),
                None if self.app.is_link() => None,
                None => return self.open_link_at(id, ev.position, window, cx),
            };
            if let Some(link) = link {
                let checked = self.link_checked(id, &link);
                self.open_link(link.uri, checked, window, cx);
            }
            return;
        }
        if self.reports(id, &ev.modifiers) {
            self.report(id, "press", button(ev.button), ev.position, &ev.modifiers);
            let pv = self.term.panes.entry(id.to_string()).or_default();
            pv.reporting = pv.reporting.or(button(ev.button)); // the first button held is the one tracked
            return;
        }
        if ev.button == MouseButton::Right {
            return self.card_click(id, ev.position, window, cx);
        }
        let Some(at) = self.pos_at(id, ev.position).filter(|_| ev.button == MouseButton::Left) else { return };
        let pv = self.term.panes.entry(id.to_string()).or_default();
        match pv.selection.as_mut() {
            Some(s) if ev.modifiers.shift => (s.head, s.dragging) = (at, true),
            _ => {
                let kind = match ev.click_count {
                    2 => Kind::Word,
                    n if n >= 3 => Kind::Line,
                    _ if ev.modifiers.alt => Kind::Block,
                    _ => Kind::Simple,
                };
                pv.selection = Some(Sel { kind, anchor: at, head: at, range: None, dragging: true });
            }
        }
        self.update_selection(id);
        cx.notify();
    }

    pub(super) fn mouse_move(&mut self, id: &str, ev: &MouseMoveEvent, cx: &mut Context<Self>) {
        self.term.mouse_at = Some(ev.position);
        self.hover(id, ev.position, ev.modifiers.platform, cx);
        let pressed = ev.pressed_button.and_then(button);
        let hovering = pressed.is_none() && self.live(id).view.mouse == Some(leon_core::mouse::Report::Motion);
        let pv = self.term.panes.entry(id.to_string()).or_default();
        if let Some(b) = pv.reporting.filter(|_| pressed.is_none()) {
            // the button came up outside the pane: the program still hears it
            pv.reporting = None;
            self.report(id, "release", Some(b), ev.position, &ev.modifiers);
            return;
        }
        if pv.reporting.is_some() || (hovering && self.view_top(id).is_none()) {
            self.report(id, "move", pressed, ev.position, &ev.modifiers);
            return;
        }
        let pv = self.term.panes.entry(id.to_string()).or_default();
        let Some(s) = pv.selection.as_mut().filter(|s| s.dragging) else { return };
        if pressed != Some(Button::Left) {
            s.dragging = false; // the release happened outside the pane
            return;
        }
        let Some(at) = self.pos_at(id, ev.position) else { return };
        let pv = self.term.panes.entry(id.to_string()).or_default();
        if let Some(s) = pv.selection.as_mut().filter(|s| s.head != at) {
            s.head = at;
            self.update_selection(id);
            cx.notify();
        }
    }

    pub(super) fn mouse_up(&mut self, id: &str, ev: &MouseUpEvent, cx: &mut Context<Self>) {
        let pv = self.term.panes.entry(id.to_string()).or_default();
        if let Some(held) = pv.reporting {
            // the button that came up; the tracked one stays held while another is let go
            let up = button(ev.button);
            if up == Some(held) {
                pv.reporting = None;
            }
            self.report(id, "release", up, ev.position, &ev.modifiers);
            return;
        }
        let Some(s) = pv.selection.as_mut().filter(|s| s.dragging) else { return };
        s.dragging = false;
        // a click selects nothing but stays the anchor a shift+click extends from
        if s.range.is_some() && self.keymap.copy_on_select {
            self.copy(id, cx);
        }
        cx.notify();
    }

    /// Ask the server what the selection covers now. A simple one covers nothing until the drag
    /// leaves the cell it started on.
    fn update_selection(&mut self, id: &str) {
        let Some(s) = self.term.panes.get_mut(id).and_then(|p| p.selection.as_mut()) else { return };
        if s.kind == Kind::Simple && s.anchor == s.head {
            s.range = None;
            return;
        }
        self.ask_range(id);
    }

    /// Have the server's range for pane `id`'s selection put on it. The range is worked out here
    /// at once (a copy right after a motion has it), then This Mac's answer replaces it on a later
    /// wake; over a link only a word selection takes the host's wider answer, and only on the
    /// lines the anchor and head are on.
    pub(super) fn ask_range(&mut self, id: &str) {
        let cols = self.live(id).cols;
        let link = self.app.is_link();
        let Some(s) = self.term.panes.get_mut(id).and_then(|p| p.selection.as_mut()) else { return };
        let asked = (s.kind, s.anchor, s.head);
        let here = local_range(s, cols);
        // drawn and copyable at once (a yank may come before any answer); This Mac's answer
        // replaces it, a host's only widens a word
        s.range = Some(here);
        if link && s.kind != Kind::Word {
            return;
        }
        let args = json!({ "kind": s.kind, "anchor": s.anchor, "head": s.head, "text": false });
        let block = id.to_string();
        self.term_call_latest(&format!("selection {id}"), id, "selection", args, move |ws, got| {
            let got = got.ok().and_then(|v| serde_json::from_value::<Selected>(v).ok());
            let got = got.map(|g| Range { start: g.start, end: g.end });
            // a host's range only widens a word on the lines asked about
            let got = match link {
                false => got,
                true => Some(got.filter(|g| widens(here, *g)).unwrap_or(here)),
            };
            let still = |s: &&mut Sel| (s.kind, s.anchor, s.head) == asked;
            if let Some(s) = ws.term.panes.get_mut(&block).and_then(|p| p.selection.as_mut()).filter(still) {
                s.range = got;
            }
        });
    }

    /// Copy pane `id`'s selection to the clipboard: the server's text on This Mac (it knows which
    /// lines wrapped); over a link the text of the cells the app draws, never the host's reply.
    pub(super) fn copy(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(s) = self.term.panes.get(id).and_then(|p| p.selection.as_ref()).filter(|s| s.range.is_some()) else {
            return;
        };
        self.term.copies += 1; // a newer copy drops what an older one still fetches
        if let (true, Some(r)) = (self.app.is_link(), s.range) {
            let job = Copying { range: r, block: s.kind == Kind::Block, seq: self.term.copies };
            self.copy_drawn(id, job, HashMap::new(), COPY_FETCHES);
        } else {
            // the wake the reply brings puts it on the clipboard
            let args = json!({ "kind": s.kind, "anchor": s.anchor, "head": s.head, "text": true });
            let seq = self.term.copies;
            self.term_call_then(id, "selection", args, move |ws, got| {
                let text = got.ok().and_then(|v| serde_json::from_value::<Selected>(v).ok()).and_then(|g| g.text);
                if seq == ws.term.copies {
                    ws.term.clip = text;
                }
            });
        }
        if let Some(text) = self.term.clip.take() {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }

    /// Over a link: the text of a copy's cells, from the screen, the history cache and `got`,
    /// onto `term.clip`. Rows not held yet are fetched from the host (up to `tries` more times),
    /// and this runs again with them once they come, unless a newer copy started meanwhile.
    fn copy_drawn(&mut self, id: &str, job: Copying, mut got: HashMap<usize, Vec<Span>>, tries: u32) {
        if job.seq != self.term.copies {
            return;
        }
        let (r, live) = (job.range, self.live(id));
        // never past the screen's last row, whatever a range says
        let last = r.end.line.min(live.view.scrolled + live.lines.saturating_sub(1));
        let mut lines = Vec::new();
        for line in r.start.line..=last {
            let spans = match line.checked_sub(live.view.scrolled) {
                Some(l) => live.rows.get(l).cloned().unwrap_or_default(),
                None => match got.get(&line).cloned().or_else(|| self.cached_row(id, line).map(|r| r.spans.clone())) {
                    Some(spans) => spans,
                    None if tries == 0 => return eprintln!("leon: not copied: the host didn't send the rows"),
                    None => {
                        let block_id = id.to_string();
                        let args = self.app.lines_args(line, last + 1 - line);
                        return self.term_call_then(id, "lines", args, move |ws, reply| {
                            if let Err(e) = &reply {
                                ws.app.lines_failed(e);
                            }
                            let rows = reply.ok().and_then(|v| serde_json::from_value::<Lines>(v).ok());
                            let rows = rows.map(|l| l.rows).unwrap_or_default();
                            let wanted = rows.into_iter().filter(|row| (line..=last).contains(&row.line));
                            got.extend(wanted.map(|row| (row.line, row.spans)));
                            ws.copy_drawn(&block_id, job, got, tries - 1);
                        });
                    }
                },
            };
            if let Some((from, to)) = scrollback::covered(r.start, r.end, job.block, line, live.cols) {
                lines.push(scrollback::text_of(&spans, live.cols, from, to));
            }
        }
        self.term.clip = Some(lines.join("\n"));
    }

    /// Select everything pane `id` keeps: the scrollback and the screen.
    pub(super) fn select_all(&mut self, id: &str, cx: &mut Context<Self>) {
        let live = self.live(id);
        let (oldest, _) = scrollback::bounds(&live);
        let anchor = Pos { line: oldest, col: 0 };
        let head = Pos { line: live.view.scrolled + live.lines.saturating_sub(1), col: live.cols.saturating_sub(1) };
        let s = Sel { kind: Kind::Simple, anchor, head, range: None, dragging: false };
        self.term.panes.entry(id.to_string()).or_default().selection = Some(s);
        self.update_selection(id);
        cx.notify();
    }

    /// The range pane `id`'s selection covers now: worked out here at once, replaced by This
    /// Mac's answer (tests).
    pub fn selection_range(&self, id: &str) -> Option<(Pos, Pos)> {
        let r = self.term.panes.get(id)?.selection.as_ref()?.range?;
        Some((r.start, r.end))
    }

    /// The text of the cells pane `id` draws as selected now, a line per row.
    pub fn highlighted_text(&self, id: &str) -> Option<String> {
        let r = self.term.panes.get(id)?.selection.as_ref()?.range?;
        let block = self.term.panes.get(id)?.selection.as_ref()?.kind == Kind::Block;
        let c = self.composed(id);
        let rows = c.frame.rows.iter().enumerate().filter_map(|(i, spans)| {
            let (from, to) = scrollback::covered(r.start, r.end, block, c.top + i, c.frame.cols)?;
            Some(scrollback::text_of(spans, c.frame.cols, from, to))
        });
        Some(rows.collect::<Vec<_>>().join("\n")).filter(|t| !t.is_empty())
    }
}
