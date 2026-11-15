//! Review (skeptic): a font change gives a hidden float the rows it gets when shown, even with pane
//! headers off (a float always draws its header).

mod m20b_support;

use std::time::{Duration, Instant};

use gpui::TestAppContext;
use leon_app::embedded;
use m20b_support::*;
use serde_json::json;

#[gpui::test]
fn a_hidden_float_fits_the_font_with_headers_off(cx: &mut TestAppContext) {
    let dir = private_dir();
    let app = std::rc::Rc::new(embedded::start_at(dir.path().join("server.sock")).unwrap());
    let (ws, cx) = open_with(cx, &app, None);
    act(&ws, cx, "client.pane_header.reveal");
    assert!(!ws.read_with(cx, |ws, _| ws.settings().pane_headers), "headers off");
    let sid = sid(&ws, cx);
    let command = json!(["/bin/sh", "-c", "exec sleep 300"]);
    let layout = json!({ "block": { "label": "notes", "options": { "command": command } } });
    let r = app.call("session.new_floating", json!({ "session_id": sid, "layout": layout })).unwrap();
    let id = r["block_id"].as_str().unwrap().to_string();
    let t = term(&app, &id);
    assert!(shows(cx, |cx| drawn(cx, &format!("float-{id}"))), "the float is drawn");
    assert!(drawn(cx, &format!("header-{id}")), "a float keeps its header with headers off");
    assert!(eventually(|| t.size() != (24, 80)), "the shown float was fitted");
    let shown_small = t.size();

    app.call("session.toggle", json!({ "session_id": sid, "label": "notes" })).unwrap();
    assert!(shows(cx, |cx| !drawn(cx, &format!("float-{id}"))), "hidden");
    act(&ws, cx, "client.font.increase");
    assert!(eventually(|| t.size() != shown_small), "the hidden float was resized with the font");
    let hidden = t.size();

    app.call("session.toggle", json!({ "session_id": sid, "label": "notes" })).unwrap();
    assert!(shows(cx, |cx| drawn(cx, &format!("float-{id}"))), "shown again");
    // the shown float's canvas refits it; give that a moment to land
    let end = Instant::now() + Duration::from_secs(2);
    while Instant::now() < end && t.size() == hidden {
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(t.size(), hidden, "the size the hidden float got from the font change vs its size when shown");
    app.stop();
}
