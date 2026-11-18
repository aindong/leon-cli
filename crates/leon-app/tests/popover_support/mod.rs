//! What the popover tests share: a window whose root view holds one popover, shows it the way the
//! workspace does (`popover::show`), keeps its own focus handle (the "elsewhere" focus goes to),
//! and records the popover's events.
#![allow(dead_code)]

use gpui::{
    div, px, size, AppContext, Bounds, Context, Entity, FocusHandle, InteractiveElement, IntoElement, ParentElement,
    Pixels, Render, Styled, Subscription, TestAppContext, VisualTestContext, Window,
};
use leon_app::popover::{self, Item, Popover, PopoverEvent};
use leon_app::theme::DARK;

pub struct Host {
    pub popover: Option<Entity<Popover>>,
    pub events: Vec<PopoverEvent>,
    pub focus: FocusHandle,
    _sub: Option<Subscription>,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let shown = self.popover.as_ref().map(|p| popover::show(p, cx));
        div().size_full().track_focus(&self.focus).children(shown)
    }
}

pub fn items() -> Vec<Item> {
    ["one", "two", "three"].iter().map(|id| Item::new(*id, format!("Item {id}"), None)).collect()
}

/// A 800×600 window holding a popover anchored at `anchor`, with three items and a line of text,
/// focused when `focused`.
pub fn open_at(
    cx: &mut TestAppContext,
    anchor: Bounds<Pixels>,
    focused: bool,
) -> (Entity<Host>, &mut VisualTestContext) {
    open_with(cx, anchor, focused, items())
}

/// `open_at` with these items.
pub fn open_with(
    cx: &mut TestAppContext,
    anchor: Bounds<Pixels>,
    focused: bool,
    items: Vec<Item>,
) -> (Entity<Host>, &mut VisualTestContext) {
    let (host, cx) =
        cx.add_window_view(|_, cx| Host { popover: None, events: Vec::new(), focus: cx.focus_handle(), _sub: None });
    cx.simulate_resize(size(px(800.), px(600.)));
    // GPUI reports focus changes only in the active window (`Window::draw`'s focus event)
    cx.update(|window, _| window.activate_window());
    host.update_in(cx, |host, window, cx| {
        let p = cx.new(|cx| Popover::new(anchor, vec!["a line of text".into()], items, DARK, window, cx));
        host._sub = Some(cx.subscribe(&p, |host: &mut Host, _, ev: &PopoverEvent, cx| {
            host.events.push(ev.clone());
            if *ev == PopoverEvent::Dismissed {
                host.popover = None;
            }
            cx.notify();
        }));
        if focused {
            p.update(cx, |p, cx| p.focus(window, cx));
        }
        host.popover = Some(p);
        cx.notify();
    });
    cx.run_until_parked();
    (host, cx)
}

pub fn rect(x: f32, y: f32, w: f32, h: f32) -> Bounds<Pixels> {
    Bounds { origin: gpui::point(px(x), px(y)), size: size(px(w), px(h)) }
}

pub fn events(host: &Entity<Host>, cx: &mut VisualTestContext) -> Vec<PopoverEvent> {
    host.read_with(cx, |h, _| h.events.clone())
}

pub fn highlighted(host: &Entity<Host>, cx: &mut VisualTestContext) -> Option<usize> {
    host.read_with(cx, |h, cx| h.popover.as_ref().and_then(|p| p.read(cx).highlighted()))
}
