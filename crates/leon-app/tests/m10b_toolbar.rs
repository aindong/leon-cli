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
    assert!(!drawn(cx, "sidebar-header"), "no header row");
    // the footer came back on 2026-11-22 as buttons (Connect, Settings: footer_app.rs), not a hint
    assert!(!drawn(cx, "sidebar-hint"), "no footer hint");
    let p = center(at(cx, "toolbar-palette").unwrap());
    cx.simulate_click(p, Modifiers::none());
    assert!(ws.read_with(cx, |ws, _| ws.palette_open()), "the palette button opens the palette");
    cx.simulate_keystrokes("escape");
    let p = center(at(cx, "toolbar-new").unwrap());
    cx.simulate_click(p, Modifiers::none());
    assert_eq!(s.app.ctx().state().sessions.len(), 2, "+ makes a session");
    let p = center(at(cx, "toolbar-sidebar").unwrap());
    cx.simulate_click(p, Modifiers::none());
    assert!(!ws.read_with(cx, |ws, _| ws.sidebar_visible()), "the toggle hides the sidebar");
    assert!(drawn(cx, "toolbar-sidebar"), "and stays reachable in the titlebar");
    s.app.stop();
}
