//! Review: a pane a zoom covers comes back at its old size after the font went up and back while
//! it was covered (M9 criterion 6 after M10), and fits a bigger font when the zoom ends.
mod m9_support;
use gpui::TestAppContext;
use m9_support::*;

#[gpui::test]
fn font_back_leaves_a_pane_a_zoom_covers_at_its_size(cx: &mut TestAppContext) {
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
    let before = t.size();

    // covered: the font goes up, the zoom ends, the pane fits the bigger font
    cx.simulate_keystrokes("cmd-shift-enter");
    cx.run_until_parked();
    cx.simulate_keystrokes("cmd-=");
    cx.run_until_parked();
    cx.simulate_keystrokes("cmd-shift-enter");
    let bigger = shows(cx, |_| t.size() != before);
    let at_bigger = t.size();

    // covered again: the font goes back, the zoom ends, the pane has its first size again
    cx.simulate_keystrokes("cmd-shift-enter");
    cx.run_until_parked();
    cx.simulate_keystrokes("cmd-0");
    cx.run_until_parked();
    let kept = stays(cx, &t, at_bigger);
    cx.simulate_keystrokes("cmd-shift-enter");
    let back = shows(cx, |_| t.size() == before);
    let after = t.size();
    s.app.stop();
    assert!(
        bigger && at_bigger.0 < before.0 && at_bigger.1 < before.1,
        "unzoomed, it fits the bigger font: {before:?} -> {at_bigger:?}"
    );
    assert!(kept, "covered, it keeps its size through cmd-0");
    assert!(back, "unzoomed after cmd-0, it has its first size again: {before:?} vs {after:?}");
}
