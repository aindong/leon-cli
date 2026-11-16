//! A pane a zoom covers keeps its size through a font change, and fits the new font when the zoom
//! ends.
mod m9_support;
use gpui::TestAppContext;
use m9_support::*;

#[gpui::test]
fn a_pane_a_zoom_covers_fits_the_font_when_the_zoom_ends(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    cx.simulate_keystrokes("cmd-d");
    cx.run_until_parked();
    let (w, _) = window(&s.app, &ws, cx);
    let focus = w.focus.clone().unwrap();
    let other = leaves(&w).into_iter().find(|id| *id != focus).unwrap();
    let (zoomed, covered) = (term(&s.app, &focus), term(&s.app, &other));
    assert!(shows(cx, |_| covered.size() != (24, 80)), "the split pane was fitted");
    cx.simulate_keystrokes("cmd-shift-enter");
    assert!(shows(cx, |cx| window(&s.app, &ws, cx).0.zoomed.is_some()), "zoomed");
    let split = covered.size();
    let before = zoomed.size();

    cx.simulate_keystrokes("cmd-=");
    assert!(shows(cx, |_| zoomed.size() != before), "the zoomed pane follows the font");
    let kept = stays(cx, &covered, split);
    cx.simulate_keystrokes("cmd-shift-enter");
    let fitted = shows(cx, |_| covered.size() != split);
    let got = covered.size();
    s.app.stop();
    assert!(kept, "a pane a zoom covers keeps its size through a font change");
    assert!(fitted && got.0 < split.0 && got.1 < split.1, "unzoomed, it fits the bigger font: {split:?} -> {got:?}");
}
