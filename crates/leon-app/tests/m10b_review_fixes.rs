//! Fixes from the M10b review: the tab-style toggle runs from the palette itself.

mod m10b_support;

use gpui::TestAppContext;
use leon_app::chrome::TabStyle;
use m10b_support::*;

#[gpui::test]
fn the_palette_runs_the_tab_style_toggle(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    cx.simulate_keystrokes("cmd-shift-p");
    cx.simulate_input("tab style");
    cx.simulate_keystrokes("enter");
    assert_eq!(ws.read_with(cx, |ws, _| ws.settings().tab_style), TabStyle::Top, "↩ in the palette ran it");
    assert!(!ws.read_with(cx, |ws, _| ws.palette_open()), "and closed the palette");
    assert!(drawn(cx, "tab-0") && !drawn(cx, "row-0-0"));
    s.app.stop();
}
