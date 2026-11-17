//! Motion PR 3: the palette and ⌘K's switcher fade and scale in: transparent, shrunk and lowered on
//! the first frame, between half way, shown at their place after `OVERLAY`.
mod layout_support;

use gpui::{Entity, TestAppContext, VisualTestContext};
use layout_support::*;
use leon_app::overlay_motion::{Look, OVERLAY};
use leon_app::view::Workspace;

fn look(ws: &Entity<Workspace>, cx: &mut VisualTestContext) -> Option<Look> {
    ws.read_with(cx, |ws, _| ws.overlay_look("palette"))
}

/// Opened by `open`: (first look, half way, at rest, asked a frame at rest).
fn shows(
    ws: &Entity<Workspace>,
    cx: &mut VisualTestContext,
    open: impl FnOnce(&mut VisualTestContext),
) -> (Look, Look, Look, bool) {
    open(cx);
    let first = look(ws, cx).expect("drawn from its first frame");
    frame(ws, cx, OVERLAY / 2);
    let half = look(ws, cx).unwrap();
    to_rest(ws, cx);
    let rest = look(ws, cx).unwrap();
    (first, half, rest, ws.read_with(cx, |ws, _| ws.frame_asked()))
}

#[gpui::test]
fn the_palette_and_the_switcher_fade_and_scale_in(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.set_reduce_motion(false));
    let palette = shows(&ws, cx, |cx| act(&ws, cx, "client.palette.toggle"));
    cx.simulate_keystrokes("escape");
    to_rest(&ws, cx);
    let switcher = shows(&ws, cx, |cx| cx.simulate_keystrokes("cmd-k"));
    s.app.stop();
    for (what, (first, half, rest, asked)) in [("palette", palette), ("switcher", switcher)] {
        assert!(
            first.opacity < 0.2 && first.scale < 1. && first.rise > 0.,
            "{what}: starts faint, small, low: {first:?}"
        );
        assert!(half.opacity > first.opacity && half.opacity < 1., "{what}: half way between: {half:?}");
        assert_eq!(rest, Look::SHOWN, "{what}: then shown at its place");
        assert!(!asked, "{what}: and no frame asked for");
    }
}
