//! Motion PR 1 (ADR R6): nothing animates at launch. A window opened on a session that already
//! has three panes and a drawer shown draws them at rest and asks GPUI for no frame.
mod m20b_support;

use std::rc::Rc;

use gpui::{px, size, TestAppContext};
use leon_app::embedded;
use leon_app::view::Workspace;
use m20b_support::*;
use serde_json::json;

#[gpui::test]
fn a_window_opened_on_a_busy_session_moves_nothing(cx: &mut TestAppContext) {
    let dir = private_dir();
    let app = Rc::new(embedded::start_at(dir.path().join("server.sock")).unwrap());
    let made = app.call("session.create", json!({ "label": "busy" })).unwrap();
    let sid = made["session_id"].as_str().unwrap().to_string();
    let first = made["block_id"].as_str().unwrap().to_string();
    for _ in 0..2 {
        let layout = json!({ "block": { "options": { "command": ["sh", "-c", "exec sleep 300"] } } });
        let p = json!({ "session_id": sid, "anchor_id": first, "direction": "horizontal", "side": "after",
                        "layout": layout });
        app.call("session.new_split", p).unwrap();
    }
    app.call("session.toggle", json!({ "session_id": sid, "label": "scratch", "create": true })).unwrap();
    // `open_with`, but with motion on before the first frame: this machine's Reduce Motion setting
    // must not be what keeps the launch still
    no_tailscale();
    cx.update(leon_app::view::init);
    let at = app.clone();
    let (ws, cx) = cx.add_window_view(move |window, cx| {
        let remote = Rc::new(leon_app::remote::Remote::connect(at.socket()).expect("the app attaches"));
        let mut ws = Workspace::with_settings(remote, None, window, cx);
        ws.set_reduce_motion(false);
        ws
    });
    cx.simulate_resize(size(px(1400.), px(900.)));
    cx.run_until_parked();
    let id = view_window(&app, &sid)["docked"][0]["block_id"].as_str().unwrap().to_string();
    let drawer = drawn(cx, &format!("dock-{id}"));
    let still = ws.read_with(cx, |ws, _| (ws.animating(), ws.frame_asked(), ws.sliding(), ws.fading()));
    app.stop();
    assert!(drawer, "the drawer is drawn");
    assert_eq!(still, (false, false, false, false), "nothing slides or fades, and no frame is asked for");
}
