//! GPUI phase 7 criterion 3 (.agent/plans/2026-11-19-gpui-phase7-input-handler.md): the emoji
//! picker inserts text with no key event, through the input handler, and the pane gets it.
mod focus_support;

use focus_support::*;
use gpui::{InputHandler, TestAppContext};
use leon_app::view::PaneInput;

#[gpui::test]
fn the_emoji_picker_types_into_the_pane(cx: &mut TestAppContext) {
    let s = plain_setup();
    let (ws, cx) = open(cx, &s.app);
    let t = focused(&s.app, &ws, cx);
    settle(&t);
    cx.simulate_keystrokes("e c h o space");
    let mut h = PaneInput::new(ws.clone());
    cx.update(|window, cx| h.replace_text_in_range(None, "🙂", window, cx));
    cx.simulate_keystrokes("enter");
    let printed = eventually(|| has_line(&t, "🙂"));
    s.app.stop();
    assert!(printed, "the pane printed the emoji");
}
