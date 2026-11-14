//! M10b criterion 8: one toolbar row (sidebar toggle, palette, new session) replaces the
//! sidebar's SESSIONS header and footer hint.

mod m10b_support;

use gpui::{Modifiers, TestAppContext};
use m10b_support::*;

#[gpui::test]
fn the_toolbar_buttons_work(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let bar = at(cx, "toolbar").expect("a toolbar row");
    for b in ["toolbar-sidebar", "toolbar-palette", "toolbar-new"] {
        let at = at(cx, b).unwrap_or_else(|| panic!("no {b}"));
        assert!(bar.contains(&center(at)), "{b} is in the toolbar row");
    }
    assert!(!drawn(cx, "sidebar-footer") && !drawn(cx, "sidebar-header"), "no header or footer rows");
    cx.simulate_click(center(at(cx, "toolbar-palette").unwrap()), Modifiers::none());
    assert!(ws.read_with(cx, |ws, _| ws.palette_open()), "the palette button opens the palette");
    cx.simulate_keystrokes("escape");
    cx.simulate_click(center(at(cx, "toolbar-new").unwrap()), Modifiers::none());
    assert_eq!(s.app.ctx().state().sessions.len(), 2, "+ makes a session");
    cx.simulate_click(center(at(cx, "toolbar-sidebar").unwrap()), Modifiers::none());
    assert!(!ws.read_with(cx, |ws, _| ws.sidebar_visible()), "the toggle hides the sidebar");
    assert!(drawn(cx, "toolbar-sidebar"), "and stays reachable in the titlebar");
    s.app.stop();
}
