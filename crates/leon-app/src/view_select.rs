//! The mouse on a pane: selections (drag, word, line, shift to extend, option for a rectangle),
//! reports to a program that asked for the mouse (shift selects anyway), cmd+hover and cmd+click
//! on links; copy and select-all.

use std::collections::HashMap;

use gpui::{ClipboardItem, Modifiers, MouseDownEvent};
use leon_core::mouse::Button;
use leon_core::program_status::strip_hidden;
use leon_core::screen::{Lines, Span};
use leon_core::search::{needs_confirm, Kind, Link, Pos, Range, Selected};

use super::scroll::{mods, Sel};
use super::*;
use crate::scrollback;

/// How many times a copy over a link asks the host for rows it doesn't hold (each reply is capped).
const COPY_FETCHES: u32 = 64;

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
    fn pos_at(&self, id: &str, at: gpui::Point<Pixels>) -> Option<Pos> {
        let (row, col) = self.cell_at(id, at)?;
        Some(Pos { line: self.composed(id).top + row, col })
    }

    /// Whether a press on pane `id` goes to its program: it asked for the mouse, shift isn't
    /// held, and the view shows the live screen.
    fn reports(&self, id: &str, m: &Modifiers) -> bool {
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
            // the link cmd+hover found here; on This Mac, asked now if it hasn't (over a link the
            // hover's reply may still be on its way, and nothing waits for one)
            let pos = self.pos_at(id, ev.position);
            let hovered = self
                .term
                .hovered
                .clone()
                .filter(|(b, l)| b == id && pos.is_some_and(|p| (l.start..=l.end).contains(&p)));
            let link =
                hovered.map(|(_, l)| l).or_else(|| self.link_at(id, ev.position).filter(|_| !self.app.is_link()));
            if let Some(link) = link {
                // a host's uri opens without asking only when it is the text drawn there
                let checked =
                    !self.app.is_link() || self.drawn_over(id, link.start, link.end).as_ref() == Some(&link.uri);
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

    /// Have the server's range for pane `id`'s selection put on it (over a link on a later poll,
    /// and only while the selection is still the one asked about).
    pub(super) fn ask_range(&mut self, id: &str) {
        let Some(s) = self.term.panes.get(id).and_then(|p| p.selection.as_ref()) else { return };
        let asked = (s.kind, s.anchor, s.head);
        let args = json!({ "kind": s.kind, "anchor": s.anchor, "head": s.head, "text": false });
        let block = id.to_string();
        self.term_call_latest(&format!("selection {id}"), id, "selection", args, move |ws, got| {
            let got = got.ok().and_then(|v| serde_json::from_value::<Selected>(v).ok());
            let still = |s: &&mut Sel| (s.kind, s.anchor, s.head) == asked;
            if let Some(s) = ws.term.panes.get_mut(&block).and_then(|p| p.selection.as_mut()).filter(still) {
                s.range = got.map(|g| Range { start: g.start, end: g.end });
            }
        });
    }

    /// Copy pane `id`'s selection to the clipboard: the server's text on This Mac (it knows which
    /// lines wrapped); over a link the text of the cells the app draws, never the host's reply.
    pub(super) fn copy(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(s) = self.term.panes.get(id).and_then(|p| p.selection.as_ref()).filter(|s| s.range.is_some()) else {
            return;
        };
        if let (true, Some(r)) = (self.app.is_link(), s.range) {
            self.copy_drawn(id, r, s.kind == Kind::Block, HashMap::new(), COPY_FETCHES);
        } else {
            let args = json!({ "kind": s.kind, "anchor": s.anchor, "head": s.head, "text": true });
            let got = self.term_call(id, "selection", args).ok();
            self.term.clip = got.and_then(|v| serde_json::from_value::<Selected>(v).ok()).and_then(|g| g.text);
        }
        if let Some(text) = self.term.clip.take() {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }

    /// Over a link: the text of range `r`'s cells (`block`: the same columns on each line), from
    /// the screen, the history cache and `got`, onto `term.clip`. Rows not held yet are fetched
    /// from the host (up to `tries` more times), and this runs again with them once they come.
    fn copy_drawn(&mut self, id: &str, r: Range, block: bool, mut got: HashMap<usize, Vec<Span>>, tries: u32) {
        let live = self.live(id);
        let mut lines = Vec::new();
        for line in r.start.line..=r.end.line {
            let spans = match line.checked_sub(live.view.scrolled) {
                Some(l) => live.rows.get(l).cloned().unwrap_or_default(),
                None => match got.remove(&line).or_else(|| self.cached_row(id, line).map(|r| r.spans.clone())) {
                    Some(spans) => spans,
                    None if tries == 0 => return eprintln!("leon: not copied: the host didn't send the rows"),
                    None => {
                        let block_id = id.to_string();
                        let args = json!({ "from": line, "count": r.end.line + 1 - line });
                        return self.term_call_then(id, "lines", args, move |ws, reply| {
                            let rows = reply.ok().and_then(|v| serde_json::from_value::<Lines>(v).ok());
                            let rows = rows.map(|l| l.rows).unwrap_or_default();
                            // rows already held, and only rows in the history
                            got.extend(
                                rows.into_iter().filter(|row| row.line >= line).map(|row| (row.line, row.spans)),
                            );
                            ws.copy_drawn(&block_id, r, block, got, tries - 1);
                        });
                    }
                },
            };
            if let Some((from, to)) = scrollback::covered(r.start, r.end, block, line, live.cols) {
                lines.push(scrollback::text_of(&spans, live.cols, from, to));
            }
        }
        self.term.clip = Some(lines.join("\n"));
    }

    /// The text drawn over `start..=end` of pane `id` (a link wrapped over rows is joined); None
    /// when a row of it isn't held.
    fn drawn_over(&self, id: &str, start: Pos, end: Pos) -> Option<String> {
        let live = self.live(id);
        let mut text = String::new();
        for line in start.line..=end.line {
            let row = match line.checked_sub(live.view.scrolled) {
                Some(l) => live.rows.get(l)?.clone(),
                None => self.cached_row(id, line)?.spans.clone(),
            };
            let (from, to) = scrollback::covered(start, end, false, line, live.cols)?;
            text.push_str(&scrollback::text_of(&row, live.cols, from, to));
        }
        Some(text)
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

    /// The link at `at` on pane `id`, asked of This Mac's server and waited for.
    fn link_at(&self, id: &str, at: gpui::Point<Pixels>) -> Option<Link> {
        let pos = self.pos_at(id, at)?;
        let got = self.term_call(id, "link_at", json!(pos)).ok()?;
        serde_json::from_value::<Option<Link>>(got).ok().flatten()
    }

    /// With cmd held the link under the mouse is underlined; asked again only when the mouse
    /// leaves the link it is on (over a link the answer comes on a later poll, for where the
    /// mouse was then).
    pub(super) fn hover(&mut self, id: &str, at: gpui::Point<Pixels>, cmd: bool, cx: &mut Context<Self>) {
        let pos = self.pos_at(id, at);
        let on = |(b, l): &(String, Link)| b == id && pos.is_some_and(|p| (l.start..=l.end).contains(&p));
        let pos = match (cmd, &self.term.hovered, pos) {
            (true, Some(h), _) if on(h) => return,
            (true, _, Some(pos)) => pos,
            _ => return self.set_hovered(None, cx),
        };
        let block = id.to_string();
        self.term_call_latest("link_at", id, "link_at", json!(pos), move |ws, got| {
            let link = got.ok().and_then(|v| serde_json::from_value::<Option<Link>>(v).ok()).flatten();
            ws.term.hovered = link.map(|l| (block, l));
        });
        cx.notify();
    }

    fn set_hovered(&mut self, now: Option<(String, Link)>, cx: &mut Context<Self>) {
        if now.as_ref().map(|(_, l)| &l.uri) != self.term.hovered.as_ref().map(|(_, l)| &l.uri) {
            self.term.hovered = now;
            cx.notify();
        }
    }

    /// cmd pressed or let go: the link under the mouse is underlined or not.
    pub(super) fn modifiers_changed(
        &mut self,
        ev: &gpui::ModifiersChangedEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(at) = self.term.mouse_at else { return };
        // a float over a tile: the float's grid
        match self.grid_at(at, &self.term.painted.clone()) {
            Some(id) => self.hover(&id, at, ev.modifiers.platform, cx),
            None if self.term.hovered.take().is_some() => cx.notify(),
            None => {}
        }
    }

    /// Open `uri` with the system handler: a web link that was `checked` at once, anything else
    /// after showing it.
    fn open_link(&mut self, uri: String, checked: bool, window: &mut Window, cx: &mut Context<Self>) {
        let open = self.term.opener.clone();
        let go = move |uri: &str, cx: &mut App| match &open {
            Some(f) => f(uri),
            None => cx.open_url(uri),
        };
        if checked && !needs_confirm(&uri) {
            go(&uri, cx);
            return;
        }
        let buttons = [PromptButton::Ok("Open".into()), PromptButton::Cancel("Cancel".into())];
        let (shown, plain) = (strip_hidden(&uri), strip_hidden(&leon_core::search::decoded(&uri)));
        let detail = if plain == shown { shown } else { format!("{shown}\n\nwhich opens {plain}") };
        let answer = window.prompt(PromptLevel::Info, "Open this link?", Some(&detail), &buttons, cx);
        cx.spawn(async move |_, cx| {
            if answer.await == Ok(0) {
                let _ = cx.update(|cx| go(&uri, cx));
            }
        })
        .detach();
    }

    /// Have links opened by `f` instead of the system handler (tests).
    pub fn set_opener(&mut self, f: Box<dyn Fn(&str)>) {
        self.term.opener = Some(Rc::from(f));
    }

    /// The link underlined now, while cmd is held over one.
    pub fn hovered_link(&self) -> Option<String> {
        self.term.hovered.as_ref().map(|(_, l)| l.uri.clone())
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
