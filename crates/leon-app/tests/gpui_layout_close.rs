//! Motion PR 2: closing a pane shrinks it away, drawing its last screen, while its sibling takes
//! the space; then it is gone.
mod layout_support;

use gpui::TestAppContext;
use layout_support::*;
use leon_app::layout_motion::LAYOUT;

#[gpui::test]
fn a_closed_pane_shrinks_away_showing_its_last_screen(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.set_reduce_motion(false));
    let a = tab_running(&s, &ws, cx, &quiet("a-ready"), "a-ready");
    let b = split_running(&s, &ws, cx, &a, &quiet("b-ready"), "b-ready");
    to_rest(&ws, cx);
    let (a0, b0) = (tile(cx, &a).unwrap(), tile(cx, &b).unwrap());
    let sid = sid(&ws, cx);
    close(&s, &sid, &b);
    assert!(moving(&ws, cx), "the close moves the panes");
    let (a1, b1) = (tile(cx, &a).unwrap(), tile(cx, &b).expect("still drawn as it starts to go"));
    frame(&ws, cx, LAYOUT / 2);
    let (a2, b2) = (tile(cx, &a).unwrap(), tile(cx, &b).expect("still drawn half way"));
    let text = ws.read_with(cx, |ws, _| ws.grid_drew(&b).map(|g| g.text)).unwrap_or_default();
    to_rest(&ws, cx);
    let (a3, b3) = (tile(cx, &a).unwrap(), tile(cx, &b));
    s.app.stop();
    assert_eq!((a1, b1), (a0, b0), "first frame: both where they were");
    assert!(b2.2 < b0.2 && b2.2 > 0., "half way it is narrower: {b2:?}");
    assert!(a2.2 > a0.2 && a2.2 < a3.2, "and its sibling wider: {a2:?}");
    assert!(text.contains("b-ready"), "it shows its last screen while it goes: {text:?}");
    assert!(b3.is_none(), "then it is gone");
    assert!(a3.2 > a0.2 * 2., "and its sibling fills the window: {a3:?}");
}
