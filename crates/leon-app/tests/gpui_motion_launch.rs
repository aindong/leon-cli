//! Motion PR 1 (ADR R6): nothing animates at launch. A window opened on a session that already
//! has three panes and a drawer shown draws them at rest and asks GPUI for no frame.
mod m20b_support;

use gpui::TestAppContext;
use leon_app::embedded;
use m20b_support::*;
use serde_json::json;

#[gpui::test]
fn a_window_opened_on_a_busy_session_moves_nothing(cx: &mut TestAppContext) {
    let dir = private_dir();
    let app = std::rc::Rc::new(embedded::start_at(dir.path().join("server.sock")).unwrap());
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
    let (ws, cx) = open_with(cx, &app, None);
    ws.update(cx, |ws, _| ws.set_reduce_motion(false));
    cx.run_until_parked();
    let id = view_window(&app, &sid)["docked"][0]["block_id"].as_str().unwrap().to_string();
    let drawer = drawn(cx, &format!("dock-{id}"));
    let still = ws.read_with(cx, |ws, _| (ws.animating(), ws.frame_asked(), ws.sliding(), ws.fading()));
    app.stop();
    assert!(drawer, "the drawer is drawn");
    assert_eq!(still, (false, false, false, false), "nothing slides or fades, and no frame is asked for");
}
