//! Review (motion PR 3): find opened in another pane while it is open in one fades the new bar in.
mod layout_support;

use gpui::TestAppContext;
use layout_support::*;

#[gpui::test]
fn the_find_bar_fades_in_when_it_moves_to_another_pane(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.set_reduce_motion(false));
    let a = tab_running(&s, &ws, cx, &quiet("a-ready"), "a-ready");
    let sid = sid(&ws, cx);
    let b = split_quiet(&s, &sid, &a, "b-ready");
    let target = |cx: &mut gpui::VisualTestContext| ws.read_with(cx, |ws, _| ws.target_block());
    assert!(shows(cx, |cx| target(cx).is_some()), "a pane focused");
    let first = target(cx).unwrap();
    let (other, key) = if first == a { (b, "pane.focus.right") } else { (a, "pane.focus.left") };
    to_rest(&ws, cx);
    act(&ws, cx, "client.find.open");
    to_rest(&ws, cx);
    act(&ws, cx, key);
    assert!(shows(cx, |cx| target(cx) == Some(other.clone())), "the other pane focused");
    to_rest(&ws, cx);
    act(&ws, cx, "client.find.open");
    let look = ws.read_with(cx, |ws, _| ws.overlay_look("find"));
    s.app.stop();
    assert!(look.is_some_and(|l| l.opacity < 0.2), "the bar on the other pane fades in: {look:?}");
}
