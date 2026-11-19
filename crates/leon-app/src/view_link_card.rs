//! The link card: resting the mouse on a link with no key held, or right-clicking one, shows its
//! address, Copy and Open in Browser in a popover (`popover.rs`) under it. A link is looked up once
//! per cell the mouse rests on, after `CARD_DELAY`, never per move; ⌘-hover and ⌘-click
//! (`view_select.rs`) are unchanged. The hover card takes no focus, so typing still reaches the
//! terminal, and closes when the mouse leaves the link and the card, on any key and on scroll; the
//! right-click card takes the focus for the keyboard and closes as a popover does.

use gpui::{ClipboardItem, Entity, MouseMoveEvent, Subscription};
use leon_core::program_status::strip_hidden;
use leon_core::search::{clean_uri, decoded, Link, Pos};

use super::*;
use crate::popover::{self, Item, Popover, PopoverEvent};

/// How long the mouse rests on a link before its card shows.
pub const CARD_DELAY: Duration = Duration::from_millis(450);
/// The most of an address the card shows, in characters (about nine lines): a link can be as
/// long as its program likes, and the card must leave its host and items in view.
const ADDRESS_MAX: usize = 512;

/// The link card's state, in `TermUi`.
#[derive(Default)]
pub(super) struct CardUi {
    /// The `link_card` setting is off.
    pub(super) off: bool,
    /// The cell the mouse rests on, and a count each new rest bumps (an older wait drops out).
    rest: Option<(String, Pos)>,
    rests: u64,
    found: Option<Found>,
    open: Option<Card>,
    /// How many lookups the card made.
    asks: usize,
}

/// A link a lookup found, for the card to open on.
struct Found {
    pane: String,
    /// The cell asked about, the card's row.
    pos: Pos,
    link: Link,
    /// For the keyboard (a right-click).
    focus: bool,
    /// `rests` when asked: a newer rest, key or modifier since makes the answer stale.
    rests: u64,
}

struct Card {
    pane: String,
    link: Link,
    popover: Entity<Popover>,
    focused: bool,
    _events: Subscription,
}

/// The card's text: the address with hidden characters stripped; what it opens when escapes
/// change it, decoding only what follows the host (a `%2F` in user-info must not read as a path);
/// and the host on a line of its own, as punycode when not plain ASCII, whenever user-info, an
/// escape or another script could disguise it, or the address was cut.
pub fn link_card_lines(uri: &str) -> Vec<String> {
    let cut = |s: &str| match s.char_indices().nth(ADDRESS_MAX) {
        Some((i, _)) => format!("{}…", &s[..i]),
        None => s.to_string(),
    };
    let shown = strip_hidden(uri);
    // `scheme://authority` as it is, then the rest; a browser ends a web address's authority at
    // `\` too (WHATWG URL, special schemes)
    let head = shown.find("://").map_or(0, |i| {
        let from = i + 3;
        from + shown[from..].find(['/', '?', '#', '\\']).unwrap_or(shown.len() - from)
    });
    let authority = shown[..head].split_once("://").map_or("", |(_, a)| a);
    let plain = format!("{}{}", &shown[..head], decoded(&shown[head..]));
    let mut lines = vec![cut(&shown)];
    if plain != shown {
        lines.push(format!("opens {}", cut(&plain)));
    }
    let host = authority.rsplit('@').next().unwrap_or("");
    let host = host.rsplit_once(':').filter(|(_, p)| p.bytes().all(|b| b.is_ascii_digit())).map_or(host, |(h, _)| h);
    // an `@` just past a `\` reads as user-info to a person, not to a browser
    let after = shown[head..].split(['/', '?', '#']).next().unwrap_or("");
    let disguised =
        authority.contains(['@', '%']) || after.contains('@') || !authority.is_ascii() || lines[0].ends_with('…');
    if !host.is_empty() && disguised {
        let named = decoded(host);
        lines.push(format!("host {}", cut(&idna::domain_to_ascii(&named).unwrap_or(named))));
    }
    lines
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

    /// The mouse moved over pane `id`: off the open hover card's link it closes (the pane's own
    /// move, since a float or drawer blocks the mouse from the root's), then the rest.
    pub(super) fn card_move(&mut self, id: &str, ev: &MouseMoveEvent, window: &mut Window, cx: &mut Context<Self>) {
        let off = |c: &Card| {
            c.pane != id || self.grid_pos(id, ev.position).is_none_or(|p| !(c.link.start..=c.link.end).contains(&p))
        };
        if self.term.card.open.as_ref().is_some_and(|c| !c.focused && off(c)) {
            self.card_cancel(); // back on the same cell, the wait starts again
            self.close_card(window, cx);
        }
        self.card_rest(id, ev, cx);
    }

    /// Resting on a new cell with no key or button held, and no program taking the mouse, starts
    /// the wait for its card; anything else stops it.
    fn card_rest(&mut self, id: &str, ev: &MouseMoveEvent, cx: &mut Context<Self>) {
        let pos = self.pos_at(id, ev.position).filter(|_| !self.reports(id, &ev.modifiers));
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

    /// A key, a scroll, a modifier or a move that starts no rest: no card for the last rest, and
    /// any answer on its way is stale.
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
            if self.term.card.open.as_ref().is_some_and(|c| c.focused) {
                window.prevent_default();
            }
        }
    }

    /// Look up the link at `pos`; This Mac answers at once and the card opens now, a host's
    /// answer comes with a later poll (`card_found`).
    fn ask_card(&mut self, id: String, pos: Pos, focus: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.term.card.asks += 1;
        let (pane, rests) = (id.clone(), self.term.card.rests);
        self.term_call_latest("link_card", &id, "link_at", json!(pos), move |ws, got| {
            let link = got.ok().and_then(|v| serde_json::from_value::<Option<Link>>(v).ok()).flatten();
            // a host's answer gets This Mac's checks: a clean address, over the cell asked about
            let link = link.filter(|l| clean_uri(&l.uri) && (l.start..=l.end).contains(&pos));
            ws.term.card.found = link.map(|link| Found { pane, pos, link, focus, rests });
        });
        self.open_card(window, cx);
    }

    /// Each poll: a host's answer opens its card, and a card whose pane left the screen (closed,
    /// another tab) closes; both through the window's handle, which the poll doesn't have.
    pub(super) fn card_poll(&mut self, cx: &mut Context<Self>) {
        let shown = |pane: &String| self.shown().is_some_and(|(_, ids)| ids.contains(pane));
        let gone = self.term.card.open.as_ref().is_some_and(|c| !shown(&c.pane));
        if self.term.card.found.is_some() || gone {
            let (handle, this) = (self.glass.window, cx.entity().downgrade());
            cx.defer(move |cx| {
                let _ = handle.update(cx, |_, window, cx| {
                    this.update(cx, |ws, cx| {
                        if gone {
                            ws.close_card(window, cx);
                        }
                        ws.open_card(window, cx)
                    })
                });
            });
        }
    }

    fn open_card(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Found { pane, pos, link, focus, rests }) = self.term.card.found.take() else { return };
        // an answer that came after the mouse moved on or a key, or a hover's after another card
        // opened, is dropped
        if rests != self.term.card.rests || (!focus && self.term.card.open.is_some()) {
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
        let row = pos.line.checked_sub(self.composed(id).top).filter(|r| *r < p.size.0)?;
        let col = if link.start.line == pos.line { link.start.col } else { 0 };
        let origin = p.origin + gpui::point(p.cell.0 * col as f32, p.cell.1 * row as f32);
        Some(gpui::Bounds { origin, size: gpui::size(p.cell.0, p.cell.1) })
    }

    fn card_event(&mut self, ev: &PopoverEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(card) = self.close_card(window, cx) else { return };
        self.card_cancel(); // a rest still waiting opens no card after this one

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
            self.card_cancel();
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
