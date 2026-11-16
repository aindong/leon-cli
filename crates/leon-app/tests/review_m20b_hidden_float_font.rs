//! Review (skeptic): a hidden float keeps its size through a font change and fits the font when
//! shown, even with pane headers off (a float always draws its header).

mod m20b_support;

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
    let tile = term(&app, &first_tile(&app, &sid));
    let tile_before = tile.size();
    act(&ws, cx, "client.font.increase");
    assert!(shows(cx, |_| tile.size() != tile_before), "the shown tile follows the font");
    let kept = t.size();

    app.call("session.toggle", json!({ "session_id": sid, "label": "notes" })).unwrap();
    assert!(shows(cx, |cx| drawn(cx, &format!("float-{id}"))), "shown again");
    let fitted = shows(cx, |_| t.size() != shown_small);
    let got = t.size();
    app.stop();
    assert_eq!(kept, shown_small, "a hidden float keeps its size through a font change");
    assert!(
        fitted && got.0 < shown_small.0 && got.1 < shown_small.1,
        "shown, it fits the bigger font: {shown_small:?} -> {got:?}"
    );
}
