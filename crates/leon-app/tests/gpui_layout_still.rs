//! Motion PR 2: only a change of which panes show, or of the zoom, moves them. A ratio set from the
//! API (as a divider's drag lands, maybe after the mouse is up), a window resize, a sidebar toggle
//! and a window opened on a session with three panes all draw their end at once and ask for no frame.
mod layout_support;

use std::rc::Rc;

use gpui::{px, size, TestAppContext};
use layout_support::*;
use leon_app::embedded;
use leon_app::view::Workspace;
use serde_json::json;

#[gpui::test]
fn ratios_and_sizes_snap(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.set_reduce_motion(false));
    let a = tab_running(&s, &ws, cx, &quiet("a-ready"), "a-ready");
    let b = split_running(&s, &ws, cx, &a, &quiet("b-ready"), "b-ready");
    to_rest(&ws, cx);
    let sid = sid(&ws, cx);
    let still = |cx: &mut gpui::VisualTestContext| ws.read_with(cx, |ws, _| (ws.animating(), ws.frame_asked()));
    let a0 = tile(cx, &a).unwrap();
    let p = json!({ "session_id": sid, "direction": "horizontal", "before_block_id": a, "after_block_id": b,
                    "ratio": 0.3 });
    s.app.call("session.resize_pane", p).unwrap();
    assert!(shows(cx, |cx| tile(cx, &a).is_some_and(|t| t.2 < a0.2 - 20.)), "the ratio is drawn");
    let ratio = still(cx);
    cx.simulate_resize(size(px(1100.), px(800.)));
    cx.run_until_parked();
    let resized = still(cx);
    act(&ws, cx, "client.sidebar.toggle");
    let sidebar = still(cx);
    s.app.stop();
    assert_eq!(ratio, (false, false), "a ratio from the API snaps");
    assert_eq!(resized, (false, false), "a window resize snaps");
    assert_eq!(sidebar, (false, false), "a sidebar toggle snaps");
}

#[gpui::test]
fn a_window_opened_on_three_panes_moves_none(cx: &mut TestAppContext) {
    let dir = private_dir();
    let app = Rc::new(embedded::start_at(dir.path().join("server.sock")).unwrap());
    let made = app.call("session.create", json!({ "label": "busy" })).unwrap();
    let (sid, first) =
        (made["session_id"].as_str().unwrap().to_string(), made["block_id"].as_str().unwrap().to_string());
    for _ in 0..2 {
        let layout = json!({ "block": { "options": { "command": ["sh", "-c", "exec sleep 300"] } } });
        let p = json!({ "session_id": sid, "anchor_id": first, "direction": "horizontal", "side": "after",
                        "layout": layout });
        app.call("session.new_split", p).unwrap();
    }
    no_tailscale();
    cx.update(leon_app::view::init);
    let at_ = app.clone();
    let (ws, cx) = cx.add_window_view(move |window, cx| {
        let remote = Rc::new(leon_app::remote::Remote::connect(at_.socket()).expect("the app attaches"));
        let mut ws = Workspace::with_settings(remote, None, window, cx);
        ws.set_reduce_motion(false);
        ws
    });
    cx.simulate_resize(size(px(1400.), px(900.)));
    cx.run_until_parked();
    let drawn = ws.read_with(cx, |ws, _| ws.tile_order().len());
    let still = ws.read_with(cx, |ws, _| (ws.animating(), ws.frame_asked(), ws.veil()));
    app.stop();
    assert_eq!(drawn, 3, "three panes drawn");
    assert_eq!(still, (false, false, 0.), "nothing moves at launch");
}
