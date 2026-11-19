//! GPUI phase 7 criterion 4 (.agent/plans/2026-11-19-gpui-phase7-input-handler.md): marked text
//! ends, unsent, when an overlay takes the focus.
mod focus_support;

use focus_support::*;
use gpui::{InputHandler, TestAppContext};
use leon_app::view::PaneInput;

#[gpui::test]
fn an_overlay_ends_the_marked_text(cx: &mut TestAppContext) {
    let s = plain_setup();
    let (ws, cx) = open(cx, &s.app);
    let t = focused(&s.app, &ws, cx);
    settle(&t);
    let mut h = PaneInput::new(ws.clone());
    cx.update(|window, cx| h.replace_and_mark_text_in_range(None, "にほ", None, window, cx));
    cx.run_until_parked();
    let marked = drawn(cx, "marked-text");
    cx.simulate_keystrokes("cmd-shift-p");
    let who = owner(&ws, cx);
    let still = drawn(cx, "marked-text");
    let text = ws.read_with(cx, |ws, _| ws.marked_text());
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    let leaked = t.text(false).contains('に');
    s.app.stop();
    assert!(marked, "the marked text was drawn");
    assert_eq!(who, Some("palette"), "the palette took the focus");
    assert!(!still && text.is_none(), "and the marked text ended");
    assert!(!leaked, "unsent");
}
