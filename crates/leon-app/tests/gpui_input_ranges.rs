//! CodeRabbit on #95: an input method may address part of the marked text (UTF-16): marking with a
//! range replaces that part, and a commit with a range takes the rest of the marked text with it.
mod focus_support;

use focus_support::*;
use gpui::{InputHandler, TestAppContext};
use leon_app::view::PaneInput;

#[gpui::test]
fn a_range_replaces_part_of_the_marked_text(cx: &mut TestAppContext) {
    let s = plain_setup();
    let (ws, cx) = open(cx, &s.app);
    let t = focused(&s.app, &ws, cx);
    settle(&t);
    cx.simulate_keystrokes("e c h o space");
    let mut h = PaneInput::new(ws.clone());
    cx.update(|window, cx| h.replace_and_mark_text_in_range(None, "にほん", None, window, cx));
    cx.update(|window, cx| h.replace_and_mark_text_in_range(Some(0..1), "日", Some(1..1), window, cx));
    let marked = ws.read_with(cx, |ws, _| ws.marked_text());
    let selected = cx.update(|window, cx| h.selected_text_range(false, window, cx)).map(|s| s.range);
    cx.update(|window, cx| h.replace_text_in_range(Some(1..3), "本", window, cx));
    cx.simulate_keystrokes("enter");
    let printed = eventually(|| has_line(&t, "日本"));
    s.app.stop();
    assert_eq!(marked.as_deref(), Some("日ほん"), "the range's part replaced, the rest kept");
    assert_eq!(selected, Some(1..1), "the selection, within the new text, in the marked text");
    assert!(printed, "the commit took the marked text around its range: 日本");
}
