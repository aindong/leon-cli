//! Review proof (security, correctness): text through the input handler carries no control
//! characters into the pane: a commit (or a held key's text) with a CR runs nothing.
mod focus_support;

use focus_support::*;
use gpui::{InputHandler, TestAppContext};
use leon_app::view::PaneInput;

#[gpui::test]
fn committed_text_cannot_carry_control_characters(cx: &mut TestAppContext) {
    let s = plain_setup();
    let (ws, cx) = open(cx, &s.app);
    let t = focused(&s.app, &ws, cx);
    settle(&t);
    let mut h = PaneInput::new(ws.clone());
    cx.update(|window, cx| h.replace_text_in_range(None, "echo ctl-$((6*7))\r", window, cx));
    cx.run_until_parked();
    let ran = eventually(|| has_line(&t, "ctl-42"));
    s.app.stop();
    assert!(!ran, "SEC-CTL an input-handler commit with a CR ran a command in the pane");
}
