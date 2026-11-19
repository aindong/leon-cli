//! Review proof (correctness): an Option key the find bar leaves (find_key: !m.alt) went on to the
//! pane before phase 7; with the find bar focused no input handler is registered for it.
mod focus_support;

use focus_support::*;
use gpui::TestAppContext;

#[gpui::test]
fn an_option_key_the_find_bar_leaves_still_reaches_the_pane(cx: &mut TestAppContext) {
    let s = plain_setup();
    let (ws, cx) = open(cx, &s.app);
    let t = focused(&s.app, &ws, cx);
    settle(&t);
    cx.simulate_keystrokes("e c h o space");
    cx.simulate_keystrokes("cmd-f");
    assert_eq!(owner(&ws, cx), Some("find"), "the find bar has the focus");
    cx.simulate_keystrokes("alt-x->≈");
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    cx.simulate_keystrokes("enter");
    let printed = eventually(|| has_line(&t, "≈"));
    s.app.stop();
    assert!(printed, "OPTION_KEY_DROPPED: the option key the find bar left never reached the pane");
}
