//! Motion PR 3 (review): an overlay part way through its fade is drawn where it rests, only lower:
//! the hint stays bottom right, the update notice keeps its place (it narrows around its centre),
//! and the notice fades in and out like the rest.
mod layout_support;

use gpui::{TestAppContext, VisualTestContext};
use layout_support::*;
use leon_app::overlay_motion::{OVERLAY, OVERLAY_OUT};
use leon_app::updates::{How, Phase};

/// (x, y, w) of `sel` as drawn.
fn place(cx: &mut VisualTestContext, sel: &str) -> Option<(f32, f32, f32)> {
    at(cx, sel).map(|b| (f32::from(b.origin.x), f32::from(b.origin.y), f32::from(b.size.width)))
}

#[gpui::test]
fn a_fading_overlay_is_drawn_at_its_place(cx: &mut TestAppContext) {
    let s = setup_with_config("leon.bind(\"ctrl+b>c\", \"pane.split.right\")\n");
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, cx| {
        ws.set_reduce_motion(false);
        ws.sync_keymap(cx);
    });
    cx.simulate_keystrokes("ctrl-b");
    frame(&ws, cx, OVERLAY / 3);
    let hint_mid = place(cx, "key-hint");
    to_rest(&ws, cx);
    let hint_rest = place(cx, "key-hint");
    cx.simulate_keystrokes("escape");
    to_rest(&ws, cx);

    let brew = How::Brew("brew upgrade --cask aindong/leon/leon".into());
    let phase = Phase::Available { version: "9.9.9".into(), notes: "https://example.com/n".into(), how: brew };
    ws.read_with(cx, |ws, _| ws.updater().set_phase(phase));
    cx.run_until_parked();
    let first = ws.read_with(cx, |ws, _| ws.overlay_look("notice"));
    frame(&ws, cx, OVERLAY / 3);
    let notice_mid = place(cx, "update-notice");
    to_rest(&ws, cx);
    let notice_rest = place(cx, "update-notice");
    act(&ws, cx, "client.update.dismiss");
    frame(&ws, cx, OVERLAY_OUT / 2);
    let going = (ws.read_with(cx, |ws, _| ws.overlay_look("notice")), place(cx, "update-notice").is_some());
    to_rest(&ws, cx);
    let gone = place(cx, "update-notice");
    s.app.stop();

    for (what, mid, rest) in [("hint", hint_mid, hint_rest), ("notice", notice_mid, notice_rest)] {
        let ((mx, my, mw), (rx, ry, rw)) = (mid.expect(what), rest.expect(what));
        assert!((mx - rx).abs() < 0.5 && (mw - rw).abs() < 0.5, "{what}: mid fade at x {mx} w {mw}, at rest {rx} {rw}");
        assert!(my > ry && my - ry <= 8., "{what}: mid fade just lower ({my}) than at rest ({ry})");
    }
    assert!(first.is_some_and(|l| l.opacity < 0.2), "the notice fades in: {first:?}");
    assert!(going.0.is_some_and(|l| l.opacity > 0. && l.opacity < 1.) && going.1, "and out: {going:?}");
    assert_eq!(gone, None, "then is gone");
}
