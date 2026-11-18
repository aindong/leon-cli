//! What the link card tests share: the M11 helpers, a tab with a web link and a file link on it,
//! resting the mouse, and reading the card.
#![allow(dead_code)]

#[path = "../m11_support/mod.rs"]
mod m11;
pub use m11::*;

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use gpui::{Entity, Modifiers, MouseButton, Pixels, Point, VisualTestContext};
use leon_app::view::Workspace;

pub const WEB: &str = "https://example.com/page";
pub const LINKS: &str = "stty -echo; printf 'go https://example.com/page here\\nfile:///etc/hosts\\n'; sleep 100";

/// A tab showing `LINKS`: (block, the middle of the web link, a cell on its row off the link,
/// the middle of the file link), with links opened into the returned list instead of a browser.
pub struct Links {
    pub id: String,
    pub web: Point<Pixels>,
    pub off: Point<Pixels>,
    pub file: Point<Pixels>,
    pub opened: Rc<RefCell<Vec<String>>>,
}

pub fn links(s: &Setup, ws: &Entity<Workspace>, cx: &mut VisualTestContext) -> Links {
    let id = tab_running(s, ws, cx, LINKS, "file:///etc/hosts");
    let opened = Rc::new(RefCell::new(Vec::<String>::new()));
    let o = opened.clone();
    ws.update(cx, |ws, _| ws.set_opener(Box::new(move |uri: &str| o.borrow_mut().push(uri.to_string()))));
    let (web_row, file_row) =
        (row_of(ws, cx, &id, "go https://example.com/page here"), row_of(ws, cx, &id, "file:///etc/hosts"));
    Links {
        web: cell(ws, cx, &id, web_row, 8),
        off: cell(ws, cx, &id, web_row, 1),
        file: cell(ws, cx, &id, file_row, 3),
        id,
        opened,
    }
}

/// Move the mouse to `at` with `mods` held, then let `ms` pass on GPUI's clock.
pub fn rest(ws: &Entity<Workspace>, cx: &mut VisualTestContext, at: Point<Pixels>, mods: Modifiers, ms: u64) {
    cx.simulate_mouse_move(at, None::<MouseButton>, mods);
    tick(ws, cx, Duration::from_millis(ms));
}

/// The open card's text lines, or None when no card is open.
pub fn card(ws: &Entity<Workspace>, cx: &mut VisualTestContext) -> Option<Vec<String>> {
    ws.read_with(cx, |ws, cx| ws.link_card().map(|p| p.read(cx).lines().to_vec()))
}

/// The open card's items: (id, label, hint).
pub fn card_items(ws: &Entity<Workspace>, cx: &mut VisualTestContext) -> Vec<(String, String, Option<String>)> {
    ws.read_with(cx, |ws, cx| {
        let p = ws.link_card();
        p.map(|p| p.read(cx).items().iter().map(|i| (i.id.clone(), i.label.clone(), i.hint.clone())).collect())
            .unwrap_or_default()
    })
}

/// How many link lookups the card has made.
pub fn asks(ws: &Entity<Workspace>, cx: &mut VisualTestContext) -> usize {
    ws.read_with(cx, |ws, _| ws.link_card_asks())
}

/// Move onto the card's item `id` and click it.
pub fn click_item(cx: &mut VisualTestContext, sel: &'static str) {
    let b = at(cx, sel).unwrap_or_else(|| panic!("{sel} drawn"));
    cx.simulate_mouse_move(b.center(), None::<MouseButton>, Modifiers::none());
    cx.simulate_click(b.center(), Modifiers::none());
    cx.run_until_parked();
}

pub fn right_click(cx: &mut VisualTestContext, at: Point<Pixels>) {
    cx.simulate_mouse_down(at, MouseButton::Right, Modifiers::none());
    cx.simulate_mouse_up(at, MouseButton::Right, Modifiers::none());
    cx.run_until_parked();
}
