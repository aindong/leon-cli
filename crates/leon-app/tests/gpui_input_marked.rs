//! GPUI phase 7 criterion 2 (.agent/plans/2026-11-19-gpui-phase7-input-handler.md): through the
//! input handler the app registers, an input method's marked text is drawn at the pane's cursor
//! and never sent; its commit is sent once; an unmark sends nothing.
mod focus_support;

use focus_support::*;
use gpui::{InputHandler, TestAppContext};
use leon_app::view::PaneInput;

fn mark(h: &mut PaneInput, cx: &mut gpui::VisualTestContext, text: &str) {
    cx.update(|window, cx| h.replace_and_mark_text_in_range(None, text, None, window, cx));
    cx.run_until_parked();
}

#[gpui::test]
fn marked_text_shows_at_the_cursor_and_only_the_commit_is_sent(cx: &mut TestAppContext) {
    let s = plain_setup();
    let (ws, cx) = open(cx, &s.app);
    let t = focused(&s.app, &ws, cx);
    settle(&t);
    let anchor = |cx: &mut gpui::VisualTestContext| ws.read_with(cx, |ws, _| ws.ime_anchor()).expect("a cursor");
    let start = anchor(cx);
    cx.simulate_keystrokes("e c h o space");
    assert!(eventually(|| t.text(false).contains("echo")), "echo typed");
    // the app draws the pushed screen a moment after the server has it
    // all five cells of `echo ` (the pushed screen may come in pieces)
    let moved = shows(cx, |cx| anchor(cx).origin.x >= start.origin.x + start.size.width * 4.5);
    let here = anchor(cx);
    let mut h = PaneInput::new(ws.clone());
    mark(&mut h, cx, "にほ");
    mark(&mut h, cx, "にほん");
    let shown = at(cx, "marked-text");
    let text = ws.read_with(cx, |ws, _| ws.marked_text());
    let range = cx.update(|window, cx| h.marked_text_range(window, cx));
    let candidates = cx.update(|window, cx| h.bounds_for_range(0..0, window, cx));
    let hold = h.apple_press_and_hold_enabled();
    // what an input method reads back while it converts: the marked text, and the clause it selected
    cx.update(|window, cx| h.replace_and_mark_text_in_range(None, "にほん", Some(1..3), window, cx));
    let mut adjusted = None;
    let read = cx.update(|window, cx| h.text_for_range(1..3, &mut adjusted, window, cx));
    let selected = cx.update(|window, cx| h.selected_text_range(false, window, cx)).map(|s| s.range);
    let leaked = t.text(false).contains('に');
    cx.update(|window, cx| h.replace_text_in_range(None, "日本", window, cx));
    cx.run_until_parked();
    let after_commit = at(cx, "marked-text");
    cx.simulate_keystrokes("enter");
    let printed = eventually(|| has_line(&t, "日本"));
    let twice = t.text(false).contains("日本日本");
    // Escape in the candidate list: empty marked text, then unmark; the next key types alone
    cx.simulate_keystrokes("e c h o space");
    mark(&mut h, cx, "か");
    mark(&mut h, cx, "");
    cx.update(|window, cx| h.unmark_text(window, cx));
    let unmarked = ws.read_with(cx, |ws, _| ws.marked_text());
    cx.simulate_keystrokes("x enter");
    let alone = eventually(|| has_line(&t, "x"));
    s.app.stop();
    assert!(moved, "the anchor follows the cursor: {start:?} then {here:?}");
    let shown = shown.expect("the marked text is drawn");
    let off = (shown.origin.x - here.origin.x).abs().max((shown.origin.y - here.origin.y).abs());
    assert!(off < gpui::px(0.5), "at the cursor (GPUI snaps to the pixel): {shown:?} {here:?}");
    assert_eq!(text.as_deref(), Some("にほん"), "the latest marked text");
    assert_eq!(range, Some(0..3), "marked, in UTF-16");
    assert_eq!(candidates, Some(here), "the candidate window at the cursor");
    assert!(!hold, "press-and-hold off: a held key repeats");
    assert_eq!((read.as_deref(), adjusted), (Some("ほん"), None), "the marked text reads back, the range as asked");
    assert_eq!(selected, Some(1..3), "with the selection the input method set");
    assert!(!leaked, "the pane got none of the marked text");
    assert!(after_commit.is_none(), "no marked text once committed");
    assert!(printed && !twice, "the commit reached the pane once");
    assert!(unmarked.is_none() && alone, "an unmarked composition ended and sent nothing");
}
