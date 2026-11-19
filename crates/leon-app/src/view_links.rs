//! Links in a pane: cmd+hover underlines the link under the mouse, cmd+click opens it (asking
//! first unless it is checked and safe), and a host's link only as the text drawn there.

use leon_core::search::{needs_confirm, Link, Pos};

use super::*;
use crate::scrollback;

impl Workspace {
    /// Whether `link` on pane `id` opens without asking: a host's uri only when it is the text drawn there.
    pub(super) fn link_checked(&self, id: &str, link: &Link) -> bool {
        !self.app.is_link() || self.drawn_over(id, link.start, link.end).as_ref() == Some(&link.uri)
    }

    /// The text the view shows over `start..=end` of pane `id` (a link wrapped over rows is
    /// joined); None when a row of it is off the view.
    pub(super) fn drawn_over(&self, id: &str, start: Pos, end: Pos) -> Option<String> {
        let c = self.composed(id);
        let mut text = String::new();
        for line in start.line..=end.line {
            let row = c.frame.rows.get(line.checked_sub(c.top)?)?;
            let (from, to) = scrollback::covered(start, end, false, line, c.frame.cols)?;
            text.push_str(&scrollback::text_of(row, c.frame.cols, from, to));
        }
        Some(text)
    }

    /// Open the link at `at` on pane `id` (a cmd-click no hover found), once This Mac's server
    /// says which it is: the answer comes through the queue to a task, never waited for here.
    pub(super) fn open_link_at(
        &mut self,
        id: &str,
        at: gpui::Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(pos) = self.pos_at(id, at) else { return };
        let (got, asked) = (self.term_ask(id, "link_at", json!(pos)), self.app.clone());
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(v)) = got.await else { return };
            let Some(link) = serde_json::from_value::<Option<Link>>(v).ok().flatten() else { return };
            let _ = this.update_in(cx, |ws, window, cx| {
                // a late answer after the window moved to another machine opens nothing
                if Rc::ptr_eq(&ws.app, &asked) {
                    ws.open_link(link.uri, true, window, cx) // This Mac's links are trusted (`link_checked`)
                }
            });
        })
        .detach();
    }

    /// With cmd held the link under the mouse is underlined; asked again only when the mouse
    /// leaves the link it is on (the answer comes on a later wake, for where the mouse was then).
    pub(super) fn hover(&mut self, id: &str, at: gpui::Point<Pixels>, cmd: bool, cx: &mut Context<Self>) {
        let pos = self.pos_at(id, at);
        let on = |(b, l): &(String, Link)| b == id && pos.is_some_and(|p| (l.start..=l.end).contains(&p));
        let pos = match (cmd, &self.term.hovered, pos) {
            (true, Some(h), _) if on(h) => return,
            (true, _, Some(pos)) => pos,
            _ => {
                self.term.hover_at = None;
                return self.set_hovered(None, cx);
            }
        };
        let asked = (id.to_string(), pos);
        self.term.hover_at = Some(asked.clone());
        self.term_call_latest("link_at", id, "link_at", json!(pos), move |ws, got| {
            // a late answer counts only while cmd is held over the cell it was asked for
            if ws.term.hover_at.as_ref() == Some(&asked) {
                let link = got.ok().and_then(|v| serde_json::from_value::<Option<Link>>(v).ok()).flatten();
                ws.term.hovered = link.map(|l| (asked.0, l));
            }
        });
        cx.notify();
    }

    pub(super) fn set_hovered(&mut self, now: Option<(String, Link)>, cx: &mut Context<Self>) {
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
        if ev.modifiers.modified() {
            self.card_cancel();
        }
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
    pub(super) fn open_link(&mut self, uri: String, checked: bool, window: &mut Window, cx: &mut Context<Self>) {
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
        // the link card's lines: what it opens and its host, which escapes can't disguise
        let detail = super::link_card::link_card_lines(&uri).join("\n\n");
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
}
