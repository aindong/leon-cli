//! Motion PR 1 (ADR R6): while anything moves (a drawer's slide, a fade, the theme's cross-fade,
//! the overview's open and close), the workspace's render asks GPUI for the next frame; the frame
//! that draws the last motion's end asks for none.
mod m20b_support;

use std::time::Duration;

use gpui::{Entity, TestAppContext, VisualTestContext};
use leon_app::view::Workspace;
use m20b_support::*;
use serde_json::json;

const FRAME: Duration = Duration::from_millis(16);

/// Right after `start`: (animating, asked a frame); then frames until it asks for none: (frames,
/// animating, asked).
fn motion(
    ws: &Entity<Workspace>,
    cx: &mut VisualTestContext,
    start: impl FnOnce(&mut VisualTestContext),
) -> ((bool, bool), (usize, bool, bool)) {
    start(cx);
    let now = |cx: &mut VisualTestContext| ws.read_with(cx, |ws, _| (ws.animating(), ws.frame_asked()));
    let first = now(cx);
    let mut n = 0;
    while ws.read_with(cx, |ws, _| ws.frame_asked()) && n < 100 {
        frame(ws, cx, FRAME);
        n += 1;
    }
    let (animating, asked) = now(cx);
    (first, (n, animating, asked))
}

#[gpui::test]
fn every_motion_asks_for_frames_until_it_ends(cx: &mut TestAppContext) {
    let s = setup_with_config("-- mine\n");
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, cx| {
        ws.set_reduce_motion(false);
        ws.sync_keymap(cx);
    });
    // two panes at rest, then a focus move: both fade
    let pane = ws.read_with(cx, |ws, _| ws.target_block()).unwrap();
    act(&ws, cx, "pane.split.right");
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.target_block()).is_some_and(|t| t != pane)), "split");
    while ws.read_with(cx, |ws, _| ws.frame_asked()) {
        frame(&ws, cx, FRAME);
    }
    let fade = motion(&ws, cx, |cx| {
        act(&ws, cx, "pane.focus.left");
        assert!(ws.read_with(cx, |ws, _| ws.fading()), "a pane fades");
    });
    let sid = sid(&ws, cx);
    let app = s.app.clone();
    let slide = motion(&ws, cx, |cx| {
        app.call("session.toggle", json!({ "session_id": sid, "label": "scratch", "create": true })).unwrap();
        assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.sliding())), "the drawer slides");
    });
    let theme = motion(&ws, cx, |cx| {
        act(&ws, cx, "client.theme.change");
        cx.simulate_keystrokes("down");
        cx.run_until_parked();
    });
    cx.simulate_keystrokes("escape");
    while ws.read_with(cx, |ws, _| ws.frame_asked()) {
        frame(&ws, cx, FRAME); // the theme going back
    }
    let open = motion(&ws, cx, |cx| act(&ws, cx, "client.overview.toggle"));
    let close = motion(&ws, cx, |cx| act(&ws, cx, "client.overview.toggle"));
    let overview_gone = !ws.read_with(cx, |ws, _| ws.overview_open());
    s.app.stop();
    for (name, (first, (n, animating, asked))) in
        [("slide", slide), ("fade", fade), ("theme", theme), ("overview open", open), ("overview close", close)]
    {
        assert_eq!(first, (true, true), "{name}: moving, and render asked for a frame");
        assert!(n >= 2, "{name}: {n} frames");
        assert!(n < 100, "{name}: it never stopped asking");
        assert_eq!((animating, asked), (false, false), "{name}: at rest, it asks for no frame");
    }
    assert!(overview_gone, "the overview closed");
}
