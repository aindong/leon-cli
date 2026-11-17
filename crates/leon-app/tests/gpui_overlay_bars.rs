//! Motion PR 3: the find bar and a pending key sequence's hint fade in when they show and out when
//! they go.
mod layout_support;

use gpui::{Entity, TestAppContext, VisualTestContext};
use layout_support::*;
use leon_app::overlay_motion::{Look, OVERLAY, OVERLAY_OUT};
use leon_app::view::Workspace;

/// `show`, then `hide`: (the first look, shown at rest, half way out, gone at rest).
fn in_and_out(
    ws: &Entity<Workspace>,
    cx: &mut VisualTestContext,
    name: &str,
    show: impl FnOnce(&mut VisualTestContext),
    hide: impl FnOnce(&mut VisualTestContext),
) -> (Option<Look>, Option<Look>, Option<Look>, Option<Look>) {
    let look = |cx: &mut VisualTestContext| ws.read_with(cx, |ws, _| ws.overlay_look(name));
    show(cx);
    let first = look(cx);
    frame(ws, cx, OVERLAY);
    to_rest(ws, cx);
    let shown = look(cx);
    hide(cx);
    frame(ws, cx, OVERLAY_OUT / 2);
    let half = look(cx);
    to_rest(ws, cx);
    (first, shown, half, look(cx))
}

#[gpui::test]
fn the_find_bar_and_the_hint_fade_in_and_out(cx: &mut TestAppContext) {
    let s = setup_with_config("leon.bind(\"ctrl+b>c\", \"pane.split.right\")\n");
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, cx| {
        ws.set_reduce_motion(false);
        ws.sync_keymap(cx);
    });
    let find =
        in_and_out(&ws, cx, "find", |cx| act(&ws, cx, "client.find.open"), |cx| cx.simulate_keystrokes("escape"));
    let hint =
        in_and_out(&ws, cx, "hint", |cx| cx.simulate_keystrokes("ctrl-b"), |cx| cx.simulate_keystrokes("escape"));
    s.app.stop();
    for (what, (first, shown, half, gone)) in [("find bar", find), ("hint", hint)] {
        assert!(first.is_some_and(|l| l.opacity < 0.2), "{what}: starts faint: {first:?}");
        assert_eq!(shown, Some(Look::SHOWN), "{what}: then shown");
        assert!(half.is_some_and(|l| l.opacity > 0. && l.opacity < 1.), "{what}: going, half way: {half:?}");
        assert_eq!(gone, None, "{what}: then gone");
    }
}
