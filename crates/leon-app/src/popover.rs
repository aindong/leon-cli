//! A small popover: a card under a point or a rect, with a few lines of text and items to choose by
//! mouse or keyboard (Up, Down, Enter; Escape closes). It is its own view with its own focus
//! handle, emitting `Chose` or `Dismissed` for its owner to act on, as Zed's `ContextMenu` does;
//! the owner draws it with `show`, GPUI's `deferred(anchored(…))`, which paints it over the
//! window and flips it to stay inside (gpui-0.2.2/src/elements/anchored.rs, `SwitchAnchor`).
//! It dismisses itself on a press outside it, on losing the focus it was given, and on any key
//! but its own. GPUI 0.2.2 has no accessibility API: everything here is reachable by keyboard and
//! drawn in the theme's colours, which is the accessibility it can have.

use gpui::prelude::FluentBuilder;
use gpui::{
    anchored, deferred, div, point, px, rgb, App, Bounds, Context, Entity, EventEmitter, FocusHandle,
    InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Pixels, Render, SharedString,
    StatefulInteractiveElement, Styled, Subscription, Window,
};

use crate::theme::{Theme, SECONDARY_TEXT, UI_TEXT};
use crate::view::{drawing, sel};

/// The card's width; its text wraps to it.
const WIDTH: f32 = 360.;

/// A choice in a popover: `id` is what `Chose` carries, `hint` a muted note on its right (a
/// shortcut). A heading names the items under it and is never highlighted or chosen.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub id: String,
    pub label: String,
    pub hint: Option<String>,
    pub heading: bool,
}

impl Item {
    pub fn new(id: impl Into<String>, label: impl Into<String>, hint: Option<String>) -> Item {
        Item { id: id.into(), label: label.into(), hint, heading: false }
    }

    pub fn heading(label: impl Into<String>) -> Item {
        Item { id: String::new(), label: label.into(), hint: None, heading: true }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum PopoverEvent {
    Chose(String),
    Dismissed,
}

pub struct Popover {
    anchor: Bounds<Pixels>,
    lines: Vec<String>,
    items: Vec<Item>,
    highlighted: Option<usize>,
    theme: Theme,
    focus: FocusHandle,
    /// It chose or was dismissed: its owner hears one `Chose` or `Dismissed`, whatever follows.
    ended: bool,
    /// Text shown as a QR code under the lines (the web page's address).
    qr: Option<String>,
    _blur: Subscription,
}

impl EventEmitter<PopoverEvent> for Popover {}

impl Popover {
    /// A popover under `anchor` (a point is a rect of no size). It takes no focus until `focus`.
    pub fn new(
        anchor: Bounds<Pixels>,
        lines: Vec<String>,
        items: Vec<Item>,
        theme: Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Popover {
        let focus = cx.focus_handle();
        let blur = cx.on_blur(&focus, window, |p, _, cx| p.end(PopoverEvent::Dismissed, cx));
        Popover { anchor, lines, items, highlighted: None, theme, focus, ended: false, qr: None, _blur: blur }
    }

    /// Take the focus, for the keyboard: the first item is highlighted unless one already is.
    pub fn focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.highlighted = self.highlighted.or_else(|| self.items.iter().position(|i| !i.heading));
        window.focus(&self.focus);
        cx.notify();
    }

    /// Show `items` instead (a list that filled in later): the highlighted one stays highlighted
    /// while it is still there, else the first.
    pub fn set_items(&mut self, items: Vec<Item>, cx: &mut Context<Self>) {
        let was = self.highlighted.and_then(|i| self.items.get(i)).map(|i| i.id.clone());
        self.items = items;
        let first = self.items.iter().position(|i| !i.heading);
        self.highlighted = match (was, self.highlighted) {
            (Some(id), _) => self.items.iter().position(|i| !i.heading && i.id == id).or(first),
            (None, h) => h.and(first),
        };
        cx.notify();
    }

    /// Show `qr`'s text as a QR code under the lines.
    pub fn with_qr(self, qr: Option<String>) -> Popover {
        Popover { qr, ..self }
    }

    pub fn has_qr(&self) -> bool {
        self.qr.is_some()
    }

    pub fn is_focused(&self, window: &Window) -> bool {
        self.focus.is_focused(window)
    }

    pub fn anchor(&self) -> Bounds<Pixels> {
        self.anchor
    }

    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    pub fn items(&self) -> &[Item] {
        &self.items
    }

    pub fn highlighted(&self) -> Option<usize> {
        self.highlighted
    }

    pub fn set_theme(&mut self, theme: Theme, cx: &mut Context<Self>) {
        self.theme = theme;
        cx.notify();
    }

    fn choose(&mut self, i: usize, cx: &mut Context<Self>) {
        if let Some(item) = self.items.get(i).filter(|i| !i.heading) {
            self.end(PopoverEvent::Chose(item.id.clone()), cx);
        }
    }

    fn end(&mut self, ev: PopoverEvent, cx: &mut Context<Self>) {
        if !std::mem::replace(&mut self.ended, true) {
            cx.emit(ev);
        }
    }

    fn key_down(&mut self, ev: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();
        let n = self.items.len();
        match ev.keystroke.key.as_str() {
            "down" | "up" if self.items.iter().any(|i| !i.heading) => {
                // from nothing highlighted, Down starts at the top and Up at the bottom; headings are passed over
                let (by, from) = if ev.keystroke.key == "down" { (1, n - 1) } else { (n - 1, 0) };
                let mut at = self.highlighted.unwrap_or(from);
                loop {
                    at = (at + by) % n;
                    if !self.items[at].heading {
                        break;
                    }
                }
                self.highlighted = Some(at);
                cx.notify();
            }
            "enter" => match self.highlighted {
                Some(i) => self.choose(i, cx),
                None => self.end(PopoverEvent::Dismissed, cx),
            },
            _ => self.end(PopoverEvent::Dismissed, cx),
        }
    }
}

impl Render for Popover {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        drawing(cx);
        let t = &self.theme;
        let text = (!self.lines.is_empty()).then(|| {
            div()
                .px(px(10.))
                .pt(px(4.))
                .pb(px(6.))
                .mb(px(4.))
                .border_b_1()
                .border_color(rgb(t.border))
                .text_size(px(SECONDARY_TEXT))
                .text_color(rgb(t.muted))
                .children(self.lines.iter().map(|l| div().child(l.clone())))
        });
        let items = self.items.iter().enumerate().map(|(i, item)| {
            if item.heading {
                return div()
                    .id(SharedString::from(format!("popover-heading-{i}")))
                    .debug_selector(move || sel(format!("popover-heading-{i}")))
                    .mx(px(4.))
                    .px(px(6.))
                    .pt(px(if i == 0 { 2. } else { 8. }))
                    .pb(px(2.))
                    .text_size(px(SECONDARY_TEXT))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(rgb(t.muted))
                    .child(item.label.clone());
            }
            let name = format!("popover-item-{}", item.id);
            let on = self.highlighted == Some(i);
            div()
                .id(SharedString::from(name.clone()))
                .debug_selector(move || sel(name))
                .flex()
                .justify_between()
                .gap(px(16.))
                .mx(px(4.))
                .px(px(6.))
                .py(px(3.))
                .rounded(px(4.))
                .when(on, |d| d.bg(rgb(t.selected)).text_color(rgb(t.title)))
                // one highlight for the mouse and the keyboard: state the mouse sets, not a hover style
                .on_mouse_move(cx.listener(move |p, _, _, cx| {
                    if p.highlighted != Some(i) {
                        p.highlighted = Some(i);
                        cx.notify();
                    }
                }))
                .on_click(cx.listener(move |p, _, _, cx| p.choose(i, cx)))
                .child(item.label.clone())
                .children(item.hint.clone().map(|h| div().text_color(rgb(t.muted)).child(h)))
        });
        div()
            .id("popover")
            .debug_selector(|| sel("popover".into()))
            .key_context("Popover")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::key_down))
            .on_mouse_down_out(cx.listener(|p, _, _, cx| p.end(PopoverEvent::Dismissed, cx)))
            // clicks stay on the card; the wheel goes through to what is under it
            .block_mouse_except_scroll()
            .w(px(WIDTH))
            .flex()
            .flex_col()
            .py(px(4.))
            .rounded(px(6.))
            .bg(rgb(t.bg))
            .border_1()
            .border_color(rgb(t.border))
            .shadow_lg()
            .font_family(".SystemUIFont")
            .text_size(px(UI_TEXT))
            .text_color(rgb(t.text))
            .children(text)
            .children(items)
    }
}

/// Draw `popover` over everything, under its anchor rect, flipped above it (or to its left) when
/// it would leave the window. GPUI's anchored element flips around the position without its
/// offset, so the rect's height is a top padding of the anchored box instead of an offset: flipped
/// up, the box's bottom sits on the rect's top and the padding is above the card, not between.
pub fn show(popover: &Entity<Popover>, cx: &App) -> impl IntoElement {
    let a = popover.read(cx).anchor;
    let card = div().pt(a.size.height).child(popover.clone());
    deferred(anchored().position(point(a.origin.x, a.origin.y)).child(card)).with_priority(1)
}
