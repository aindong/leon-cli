//! Review: a name longer than the server takes (128 characters) is cut to fit, not silently dropped.
mod close_support;

use close_support::*;
use gpui::TestAppContext;

#[gpui::test]
fn a_long_inline_name_is_set_not_dropped(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let p = center(at(cx, "row-0-0").expect("the tab row"));
    double_click(cx, p);
    cx.simulate_keystrokes(&vec!["a"; 150].join(" "));
    cx.simulate_keystrokes("enter");
    let want = "a".repeat(128);
    let set = shows(cx, |cx| ws.read_with(cx, |ws, _| ws.tab_titles()) == [want.clone()]);
    s.app.stop();
    assert!(set, "a 150-character name was dropped");
}
