//! CodeRabbit on #95: a composition an overlay ended stays unsent, but a key typed afterwards is
//! the user's: text equal to the key just pressed reaches the pane; a commit with no key (the old
//! composition's) doesn't.
mod focus_support;

use focus_support::*;
use gpui::{InputHandler, TestAppContext};
use leon_app::view::PaneInput;

#[gpui::test]
fn a_key_after_an_ended_composition_is_typed(cx: &mut TestAppContext) {
    let s = plain_setup();
    let (ws, cx) = open(cx, &s.app);
    let t = focused(&s.app, &ws, cx);
    settle(&t);
    let mut h = PaneInput::new(ws.clone());
    let compose_and_cancel = |h: &mut PaneInput, cx: &mut gpui::VisualTestContext| {
        cx.update(|window, cx| h.replace_and_mark_text_in_range(None, "にほ", None, window, cx));
        cx.run_until_parked();
        cx.simulate_keystrokes("cmd-shift-p");
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
    };
    compose_and_cancel(&mut h, cx);
    cx.simulate_keystrokes("e c h o space x y");
    compose_and_cancel(&mut h, cx);
    cx.update(|window, cx| h.replace_text_in_range(None, "日本", window, cx)); // its late commit
    cx.simulate_keystrokes("z enter");
    let printed = eventually(|| has_line(&t, "xyz"));
    s.app.stop();
    assert!(printed, "the keys typed after a cancel reached the pane, the late commit didn't:\n{}", t.text(false));
}
