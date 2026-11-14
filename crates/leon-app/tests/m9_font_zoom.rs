//! Font size resizes the panes a zoom hides, not only the zoomed one (criterion 6: every pane).
mod m9_support;
use gpui::TestAppContext;
use m9_support::*;

#[gpui::test]
fn font_size_resizes_panes_hidden_by_a_zoom(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    cx.simulate_keystrokes("cmd-d");
    cx.run_until_parked();
    let (w, _) = window(&s.app, &ws, cx);
    let focus = w.focus.clone().unwrap();
    let other = leaves(&w).into_iter().find(|id| *id != focus).unwrap();
    cx.simulate_keystrokes("cmd-shift-enter");
    cx.run_until_parked();
    assert!(window(&s.app, &ws, cx).0.zoomed.is_some(), "zoomed");
    let before = term(&s.app, &other).size();
    cx.simulate_keystrokes("cmd-=");
    cx.run_until_parked();
    let after = term(&s.app, &other).size();
    s.app.stop();
    assert!(after.0 < before.0 && after.1 < before.1, "pane hidden by zoom not resized: {before:?} -> {after:?}");
}
