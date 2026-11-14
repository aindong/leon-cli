//! Review: a pane hidden by a zoom follows the font back to its old size (M9 criterion 6 after M10).
mod m9_support;
use gpui::TestAppContext;
use m9_support::*;

#[gpui::test]
fn font_back_resizes_a_pane_hidden_by_a_zoom(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    cx.simulate_keystrokes("cmd-d");
    cx.run_until_parked();
    let (w, _) = window(&s.app, &ws, cx);
    let focus = w.focus.clone().unwrap();
    let other = leaves(&w).into_iter().find(|id| *id != focus).unwrap();
    let t = term(&s.app, &other);
    // the app has received the other pane's screen at its split size
    let arrived = shows(cx, |cx| {
        let rows = t.size().0 as usize;
        ws.read_with(cx, |ws, _| ws.screen_text(&other)).is_some_and(|x| x.lines().count() == rows)
    });
    assert!(arrived, "the split pane's screen never arrived");
    cx.simulate_keystrokes("cmd-shift-enter");
    cx.run_until_parked();
    assert!(window(&s.app, &ws, cx).0.zoomed.is_some(), "zoomed");
    let before = t.size();
    cx.simulate_keystrokes("cmd-=");
    cx.run_until_parked();
    assert_ne!(t.size(), before, "a bigger font resized the hidden pane");
    cx.simulate_keystrokes("cmd-0");
    cx.run_until_parked();
    let after = t.size();
    s.app.stop();
    assert_eq!(after, before, "cmd-0 leaves the pane hidden by a zoom at the bigger font's size");
}
