//! ADR R5 (docs/architecture/gpui.md): the workspace's `render` only draws. A frame GPUI draws
//! with nothing changed (a refresh: a hover style, a tooltip, a resize) asks the server for nothing
//! and fetches nothing; the model work happens when the workspace is told something changed.
mod m10b_support;

use std::rc::Rc;

use gpui::{px, size, TestAppContext};
use leon_app::remote::Remote;
use leon_app::view::{self, Workspace};
use m10b_support::*;

#[gpui::test]
fn two_frames_with_nothing_changed_ask_the_server_nothing(cx: &mut TestAppContext) {
    let s = setup();
    no_tailscale();
    cx.update(view::init);
    let socket = s.app.socket().to_path_buf();
    let remote = Rc::new(Remote::connect(&socket).expect("the app attaches"));
    let held = remote.clone();
    let (ws, cx) = cx.add_window_view(move |window, cx| Workspace::new(held, window, cx));
    cx.simulate_resize(size(px(1400.), px(900.)));
    cx.run_until_parked();
    settle(&focused(&s.app, &ws, cx));
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.watching().len()) == 1), "the pane is watched");
    cx.run_until_parked();

    let asks = remote.asks();
    let fetches = ws.read_with(cx, |ws, _| ws.fetches());
    let renders = ws.read_with(cx, |ws, _| ws.renders());
    // each update draws the refreshed window when it ends; the poll's task does not run in between
    cx.update(|window, _| window.refresh());
    cx.update(|window, _| window.refresh());
    let drew = ws.read_with(cx, |ws, _| ws.renders()) - renders;
    let (asked, fetched) = (remote.asks() - asks, ws.read_with(cx, |ws, _| ws.fetches()) - fetches);
    s.app.stop();
    assert_eq!(drew, 2, "two frames were drawn");
    assert_eq!(asked, 0, "two frames with nothing changed called the server {asked} times");
    assert_eq!(fetched, 0, "two frames with nothing changed fetched history {fetched} times");
}

/// The work render did moved to an observer of the workspace's notify: one notify runs it once,
/// before the one frame it asks for, and it never notifies again itself.
#[gpui::test]
fn one_notify_brings_the_model_up_to_date_once(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    settle(&focused(&s.app, &ws, cx));
    cx.run_until_parked();
    let counts = |cx: &mut gpui::VisualTestContext| ws.read_with(cx, |ws, _| (ws.prepares(), ws.renders()));
    let (prepares, renders) = counts(cx);
    ws.update(cx, |_, cx| {
        cx.notify();
        cx.notify();
    });
    let (p, r) = counts(cx);
    s.app.stop();
    assert_eq!((p - prepares, r - renders), (1, 1), "one notify (twice in one update): one prepare, one frame");
}

/// What `render` must not call (R5): the model work it did before the move, now in `prepare` or
/// after the frame. A frame with nothing changed can't show every one of them missing (most do
/// nothing at idle), so the source is read too.
#[test]
fn the_workspace_render_calls_no_model_work() {
    let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/view_panes.rs")).unwrap();
    let start = src.find("impl Render for Workspace").expect("the workspace's render");
    let body = &src[start..];
    let body = &body[..body[1..].find("\nimpl ").map_or(body.len(), |i| i + 1)];
    let calls = [
        "note_focus(",
        "note_slides(",
        "settle_overview(",
        "sync_pane(",
        "run_find(",
        "settle(",
        "note_recent(",
        "note_fades(",
        "watch_screens(",
        "self.drew_frame(",
        "DRAWN",
        ".retain(",
        "find = None",
    ];
    let found: Vec<&str> = calls.iter().copied().filter(|c| body.contains(c)).collect();
    assert!(found.is_empty(), "Workspace::render still calls {found:?}");
}
