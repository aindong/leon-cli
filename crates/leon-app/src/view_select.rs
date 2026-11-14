//! The mouse on a pane: selections (drag, word, line, shift to extend, option for a rectangle),
//! reports to a program that asked for the mouse (shift selects anyway), cmd+hover and cmd+click
//! on links; copy and select-all.

use gpui::{ClipboardItem, Modifiers, MouseDownEvent};
use leon_core::mouse::Button;
use leon_core::search::{needs_confirm, Kind, Link, Pos, Range, Selected};

use super::scroll::{mods, Sel};
use super::*;
use crate::scrollback;

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
            if let Some(link) = self.link_at(id, ev.position) {
                self.open_link(link.uri, window, cx);
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
        if let Some(b) = pv.reporting.take() {
            self.report(id, "release", Some(b), ev.position, &ev.modifiers);
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
        let Some(s) = self.term.panes.get(id).and_then(|p| p.selection.as_ref()) else { return };
        let range = if s.kind == Kind::Simple && s.anchor == s.head {
            None
        } else {
            self.selected(id, s, false).map(|g| Range { start: g.start, end: g.end })
        };
        if let Some(s) = self.term.panes.get_mut(id).and_then(|p| p.selection.as_mut()) {
            s.range = range;
        }
    }

    /// What the server makes of selection `s` on pane `id`, with its text if asked.
    fn selected(&self, id: &str, s: &Sel, text: bool) -> Option<Selected> {
        let args = json!({ "kind": s.kind, "anchor": s.anchor, "head": s.head, "text": text });
        serde_json::from_value(self.term_call(id, "selection", args).ok()?).ok()
    }

    /// Copy pane `id`'s selection to the clipboard.
    pub(super) fn copy(&self, id: &str, cx: &mut Context<Self>) {
        let Some(s) = self.term.panes.get(id).and_then(|p| p.selection.as_ref()).filter(|s| s.range.is_some()) else {
            return;
        };
        if let Some(text) = self.selected(id, s, true).and_then(|g| g.text) {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
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

    fn link_at(&self, id: &str, at: gpui::Point<Pixels>) -> Option<Link> {
        let pos = self.pos_at(id, at)?;
        let got = self.term_call(id, "link_at", json!(pos)).ok()?;
        serde_json::from_value::<Option<Link>>(got).ok().flatten()
    }

    /// With cmd held the link under the mouse is underlined; asked again only when the mouse
    /// leaves the link it is on.
    pub(super) fn hover(&mut self, id: &str, at: gpui::Point<Pixels>, cmd: bool, cx: &mut Context<Self>) {
        let pos = self.pos_at(id, at);
        let on = |(b, l): &(String, Link)| b == id && pos.is_some_and(|p| (l.start..=l.end).contains(&p));
        let now = match (cmd, &self.term.hovered) {
            (false, _) => None,
            (true, Some(h)) if on(h) => return,
            (true, _) => self.link_at(id, at).map(|l| (id.to_string(), l)),
        };
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
        let painted = self.term.painted.borrow().clone();
        let inside = |p: &super::scroll::Painted| {
            let (w, h) = (p.cell.0 * p.size.1 as f32, p.cell.1 * p.size.0 as f32);
            Bounds::new(p.origin, size(w, h)).contains(&at)
        };
        match painted.iter().find(|(_, p)| inside(p)) {
            Some((id, _)) => self.hover(&id.clone(), at, ev.modifiers.platform, cx),
            None if self.term.hovered.take().is_some() => cx.notify(),
            None => {}
        }
    }

    /// Open `uri` with the system handler: a web link at once, anything else after showing it.
    fn open_link(&mut self, uri: String, window: &mut Window, cx: &mut Context<Self>) {
        let open = self.term.opener.clone();
        let go = move |uri: &str, cx: &mut App| match &open {
            Some(f) => f(uri),
            None => cx.open_url(uri),
        };
        if !needs_confirm(&uri) {
            go(&uri, cx);
            return;
        }
        let buttons = [PromptButton::Ok("Open".into()), PromptButton::Cancel("Cancel".into())];
        let plain = leon_core::search::decoded(&uri);
        let detail = if plain == uri { uri.clone() } else { format!("{uri}\n\nwhich opens {plain}") };
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
