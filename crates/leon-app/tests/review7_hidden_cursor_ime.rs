//! Review proof (skeptic): a program that hides the terminal cursor (an ink CLI, htop) still gets
//! its input method's marked text drawn at the cursor's place.
mod focus_support;

use focus_support::*;
use gpui::{InputHandler, TestAppContext};
use leon_app::view::PaneInput;

#[gpui::test]
fn marked_text_shows_while_the_program_hides_the_cursor(cx: &mut TestAppContext) {
    let s = plain_setup();
    let (ws, cx) = open(cx, &s.app);
    let t = focused(&s.app, &ws, cx);
    settle(&t);
    let anchor = |cx: &mut gpui::VisualTestContext| ws.read_with(cx, |ws, _| ws.ime_anchor());
    let start = anchor(cx).expect("a cursor at the prompt");
    // what a full-screen program does: hide the cursor (DECTCEM off), print, wait for input
    t.write(b"printf '\\033[?25l'; echo hidden-now; sleep 30\r").unwrap();
    assert!(eventually(|| has_line(&t, "hidden-now")), "the program ran:\n{}", t.text(false));
    // two rows under the prompt: past `hidden-now`, so the cursor is hidden by then
    let below = shows(cx, |cx| anchor(cx).is_some_and(|b| b.origin.y >= start.origin.y + start.size.height * 1.5));
    let mut h = PaneInput::new(ws.clone());
    cx.update(|window, cx| h.replace_and_mark_text_in_range(None, "にほ", None, window, cx));
    cx.run_until_parked();
    let shown = at(cx, "marked-text");
    let candidates = cx.update(|window, cx| h.bounds_for_range(0..0, window, cx));
    s.app.stop();
    assert!(shown.is_some(), "the marked text is drawn while the program hides the cursor (anchor followed: {below})");
    assert!(candidates.is_some(), "the candidate window has a place to go");
    assert!(below, "the anchor is the hidden cursor's cell");
}
