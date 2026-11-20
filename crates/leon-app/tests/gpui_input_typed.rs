//! GPUI phase 7 criterion 1 (.agent/plans/2026-11-19-gpui-phase7-input-handler.md): typed text
//! goes through the input handler, as an input method needs every key it composes from:
//! `key_down` leaves it, and Enter still goes from `key_down`.
mod focus_support;

use focus_support::*;
use gpui::TestAppContext;

#[gpui::test]
fn typed_text_goes_through_the_input_handler(cx: &mut TestAppContext) {
    let s = plain_setup();
    let (ws, cx) = open(cx, &s.app);
    let t = focused(&s.app, &ws, cx);
    settle(&t);
    let before = ws.read_with(cx, |ws, _| ws.input_commits());
    cx.simulate_keystrokes("e c h o space あ い");
    // an arrow and a ctrl chord are key_down's, not the handler's
    cx.simulate_keystrokes("left right ctrl-e");
    let typed = ws.read_with(cx, |ws, _| ws.input_commits()) - before;
    cx.simulate_keystrokes("enter");
    let printed = eventually(|| has_line(&t, "あい"));
    let after_enter = ws.read_with(cx, |ws, _| ws.input_commits()) - before;
    s.app.stop();
    assert_eq!(typed, 7, "each typed character went through the input handler (e c h o, space, あ, い)");
    assert!(printed, "the pane printed あい");
    assert_eq!(after_enter, 7, "Enter went from key_down, not the handler");
}
