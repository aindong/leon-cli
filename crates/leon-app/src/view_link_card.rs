//! The link card: resting the mouse on a link with no key held, or right-clicking one, shows its
//! address, Copy and Open in Browser in a popover (`popover.rs`) under it. A link is looked up once
//! per cell the mouse rests on, after `CARD_DELAY`, never per move; ⌘-hover and ⌘-click
//! (`view_select.rs`) are unchanged. The hover card takes no focus, so typing still reaches the
//! terminal, and closes when the mouse leaves the link and the card, on any key and on scroll; the
//! right-click card takes the focus for the keyboard and closes as a popover does.

use gpui::{ClipboardItem, Entity, MouseMoveEvent, Subscription};
use leon_core::program_status::strip_hidden;
use leon_core::search::{decoded, Link, Pos};

use super::*;
use crate::popover::{self, Item, Popover, PopoverEvent};

/// How long the mouse rests on a link before its card shows.
pub const CARD_DELAY: Duration = Duration::from_millis(450);
/// The most of an address the card shows, in characters: a link can be as long as its program
/// likes.
const ADDRESS_MAX: usize = 2048;

/// The link card's state, in `TermUi`.
#[derive(Default)]
pub(super) struct CardUi {
    /// The `link_card` setting is off.
    pub(super) off: bool,
    /// The cell the mouse rests on, and a count each new rest bumps (an older wait drops out).
    rest: Option<(String, Pos)>,
    rests: u64,
    /// A link a lookup found, for the card to open on: (pane, the cell asked, link, for the keyboard).
    found: Option<(String, Pos, Link, bool)>,
    open: Option<Card>,
    /// How many lookups the card made.
    asks: usize,
}

struct Card {
    pane: String,
    link: Link,
    popover: Entity<Popover>,
    focused: bool,
    _events: Subscription,
}

/// The card's text: the address with hidden characters stripped, and what it opens when that
/// differs (an encoded address), each cut to `ADDRESS_MAX`.
pub fn link_card_lines(uri: &str) -> Vec<String> {
    let cut = |s: String| match s.char_indices().nth(ADDRESS_MAX) {
        Some((i, _)) => format!("{}…", &s[..i]),
        None => s,
    };
    let (shown, plain) = (strip_hidden(uri), decoded(uri));
    let opens = (plain != shown).then(|| format!("opens {}", cut(plain)));
    std::iter::once(cut(shown)).chain(opens).collect()
}

impl Workspace {
    /// The open link card.
    pub fn link_card(&self) -> Option<Entity<Popover>> {
        self.term.card.open.as_ref().map(|c| c.popover.clone())
    }

    /// Whether the workspace has the keyboard (tests: a card gives it back as it closes).
    pub fn has_focus(&self, window: &Window) -> bool {
        self.focus.is_focused(window)
    }

    /// How many links the card has looked up.
    pub fn link_card_asks(&self) -> usize {
        self.term.card.asks
    }

    /// The mouse moved over pane `id`: resting on a new cell with no key or button held starts
    /// the wait for its card; a key held stops it.
    pub(super) fn card_rest(&mut self, id: &str, ev: &MouseMoveEvent, cx: &mut Context<Self>) {
        let pos = self.pos_at(id, ev.position);
        let card = &mut self.term.card;
        let Some(pos) = pos.filter(|_| !card.off && !ev.modifiers.modified() && ev.pressed_button.is_none()) else {
            return self.card_cancel();
        };
        let on_card = card.open.as_ref().is_some_and(|c| c.pane == id && (c.link.start..=c.link.end).contains(&pos));
        let here = Some((id.to_string(), pos));
        if on_card || card.rest == here {
            return;
        }
        (card.rest, card.rests) = (here, card.rests + 1);
        let (n, handle) = (card.rests, self.glass.window);
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(CARD_DELAY).await;
            let _ = handle.update(cx, |_, window, cx| this.update(cx, |ws, cx| ws.rested(n, window, cx)));
        })
        .detach();
    }

    /// A key was pressed with the mouse resting: no card for this rest.
    pub(super) fn card_cancel(&mut self) {
        self.term.card.rest = None;
        self.term.card.rests += 1;
    }

    /// The wait for rest `n` is over: look the cell up if the mouse is still on it and no card is
    /// open (a right-click during the wait opened one).
    fn rested(&mut self, n: u64, window: &mut Window, cx: &mut Context<Self>) {
        let card = &self.term.card;
        let Some((id, pos)) = card.rest.clone().filter(|_| card.rests == n && card.open.is_none()) else { return };
        if self.grid_pos(&id, window.mouse_position()) == Some(pos) {
            self.ask_card(id, pos, false, window, cx);
        }
    }

    /// A right-click on pane `id` with no program taking the mouse: the card of the link there,
    /// at once and for the keyboard.
    pub(super) fn card_click(
        &mut self,
        id: &str,
        at: gpui::Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(pos) = self.pos_at(id, at).filter(|_| !self.term.card.off) {
            self.close_card(window, cx);
            self.ask_card(id.to_string(), pos, true, window, cx);
            // the root takes the focus on a press it isn't told to leave (gpui's div.rs, `track_focus`)
            window.prevent_default();
        }
    }

    /// Look up the link at `pos`; This Mac answers at once and the card opens now, a host's
    /// answer comes with a later poll (`card_found`).
    fn ask_card(&mut self, id: String, pos: Pos, focus: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.term.card.asks += 1;
        let pane = id.clone();
        self.term_call_latest("link_card", &id, "link_at", json!(pos), move |ws, got| {
            let link = got.ok().and_then(|v| serde_json::from_value::<Option<Link>>(v).ok()).flatten();
            ws.term.card.found = link.map(|l| (pane, pos, l, focus));
        });
        self.open_card(window, cx);
    }

    /// The poll applied a host's answer: open its card, through the window's handle.
    pub(super) fn card_found(&mut self, cx: &mut Context<Self>) {
        if self.term.card.found.is_some() {
            let (handle, this) = (self.glass.window, cx.entity().downgrade());
            cx.defer(move |cx| {
                let _ = handle.update(cx, |_, window, cx| this.update(cx, |ws, cx| ws.open_card(window, cx)));
            });
        }
    }

    fn open_card(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((pane, pos, link, focus)) = self.term.card.found.take() else { return };
        // a hover's answer that came after the mouse moved on, or after another card opened, is dropped
        if !focus && (self.term.card.open.is_some() || self.term.card.rest.as_ref() != Some(&(pane.clone(), pos))) {
            return;
        }
        let Some(anchor) = self.card_anchor(&pane, pos, &link) else { return };
        let items = vec![Item::new("copy", "Copy", None), Item::new("open", "Open in Browser", Some("⌘Click".into()))];
        let theme = self.theme;
        let p = cx.new(|cx| Popover::new(anchor, link_card_lines(&link.uri), items, theme, window, cx));
        let events = cx.subscribe_in(&p, window, |ws, _, ev: &PopoverEvent, window, cx| ws.card_event(ev, window, cx));
        if focus {
            p.update(cx, |p, cx| p.focus(window, cx));
        }
        self.term.card.open = Some(Card { pane, link, popover: p, focused: focus, _events: events });
        cx.notify();
    }

    /// Under the link's first cell on the row the mouse is on (its row's start when the link
    /// began on a row above).
    fn card_anchor(&self, id: &str, pos: Pos, link: &Link) -> Option<gpui::Bounds<Pixels>> {
        let p = self.term.painted.borrow().get(id).copied()?;
        let row = pos.line.checked_sub(self.composed(id).top)?;
        let col = if link.start.line == pos.line { link.start.col } else { 0 };
        let origin = p.origin + gpui::point(p.cell.0 * col as f32, p.cell.1 * row as f32);
        Some(gpui::Bounds { origin, size: gpui::size(p.cell.0, p.cell.1) })
    }

    fn card_event(&mut self, ev: &PopoverEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(card) = self.close_card(window, cx) else { return };
        match ev {
            PopoverEvent::Chose(what) if what == "copy" => {
                cx.write_to_clipboard(ClipboardItem::new_string(card.link.uri));
            }
            PopoverEvent::Chose(what) if what == "open" => {
                let checked = self.link_checked(&card.pane, &card.link);
                self.open_link(card.link.uri, checked, window, cx);
            }
            _ => {}
        }
    }

    /// Close the card, giving the focus back when it had it (a click on a hover card focuses it).
    fn close_card(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<Card> {
        let card = self.term.card.open.take()?;
        if card.focused || card.popover.read(cx).is_focused(window) {
            window.focus(&self.focus);
        }
        cx.notify();
        Some(card)
    }

    /// A key or a scroll: the hover card closes (a focused card has the keys itself).
    pub(super) fn close_hover_card(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.card_cancel();
        if self.term.card.open.as_ref().is_some_and(|c| !c.focused) {
            self.close_card(window, cx);
        }
    }

    /// The mouse moved anywhere but on the card (which blocks the mouse): off its link, the hover
    /// card closes.
    pub(super) fn card_leave(&mut self, at: gpui::Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(c) = self.term.card.open.as_ref().filter(|c| !c.focused) else { return };
        let on = self.grid_pos(&c.pane, at).is_some_and(|p| (c.link.start..=c.link.end).contains(&p));
        if !on {
            self.close_card(window, cx);
        }
    }

    /// The cell under `at` in pane `id`, if `at` is on its grid (`pos_at` clamps to the edge).
    fn grid_pos(&self, id: &str, at: gpui::Point<Pixels>) -> Option<Pos> {
        let p = self.term.painted.borrow().get(id).copied()?;
        let end = p.origin + gpui::point(p.cell.0 * p.size.1 as f32, p.cell.1 * p.size.0 as f32);
        let inside = at.x >= p.origin.x && at.y >= p.origin.y && at.x < end.x && at.y < end.y;
        inside.then(|| self.pos_at(id, at)).flatten()
    }

    /// The card over everything, when one is open.
    pub(super) fn render_card(&self, cx: &App) -> Option<impl IntoElement> {
        self.term.card.open.as_ref().map(|c| popover::show(&c.popover, cx))
    }
}
