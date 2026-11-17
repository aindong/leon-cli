//! Motion PR 3: the sidebar switching to the Activity list (and back) fades the list it switches to
//! in.
mod layout_support;

use gpui::TestAppContext;
use layout_support::*;
use leon_app::overlay_motion::{Look, OVERLAY};

#[gpui::test]
fn the_activity_list_fades_in(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.set_reduce_motion(false));
    let look = |cx: &mut gpui::VisualTestContext| ws.read_with(cx, |ws, _| ws.overlay_look("list"));
    let before = look(cx);
    act(&ws, cx, "client.sidebar.activity.toggle");
    let first = look(cx);
    frame(&ws, cx, OVERLAY / 2);
    let half = look(cx);
    to_rest(&ws, cx);
    let rest = look(cx);
    act(&ws, cx, "client.sidebar.activity.toggle");
    let back = look(cx);
    to_rest(&ws, cx);
    s.app.stop();
    assert_eq!(before, Some(Look::SHOWN), "the sessions list at rest");
    assert!(first.is_some_and(|l| l.opacity < 0.2), "the Activity list starts faint: {first:?}");
    assert!(half.is_some_and(|l| l.opacity > 0.2 && l.opacity < 1.), "half way between: {half:?}");
    assert_eq!(rest, Some(Look::SHOWN), "then shown");
    assert!(back.is_some_and(|l| l.opacity < 0.2), "switching back fades the sessions list in: {back:?}");
}
