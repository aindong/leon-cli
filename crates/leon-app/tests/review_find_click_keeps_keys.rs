//! Review proof (correctness): a click that changes nothing (the sidebar's blank space) leaves an
//! open find bar its keys: the workspace taking the focus hands it on to the top open overlay.
mod focus_support;

use focus_support::*;
use gpui::{point, px, Modifiers, TestAppContext};

#[gpui::test]
fn a_click_on_the_blank_sidebar_leaves_the_find_bar_its_keys(cx: &mut TestAppContext) {
    let s = plain_setup();
    let (ws, cx) = open(cx, &s.app);
    settle(&focused(&s.app, &ws, cx));
    // a click lands on an active window (GPUI tells focus listeners nothing about an inactive one)
    cx.update(|window, _| window.activate_window());
    cx.simulate_keystrokes("cmd-f");
    assert_eq!(owner(&ws, cx), Some("find"), "the find bar has the focus");
    let side = at(cx, "sidebar").expect("the sidebar");
    let blank = point(side.origin.x + px(20.), side.origin.y + side.size.height - px(20.));
    cx.simulate_click(blank, Modifiers::none());
    cx.run_until_parked();
    cx.simulate_keystrokes("z q");
    cx.run_until_parked();
    let q = ws.read_with(cx, |ws, _| ws.find_query());
    let o = owner(&ws, cx);
    s.app.stop();
    assert_eq!(q.as_deref(), Some("zq"), "typed text edits the open find bar's query (focus {o:?})");
}
