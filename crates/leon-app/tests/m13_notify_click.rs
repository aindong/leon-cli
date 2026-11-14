//! M13 criterion 14: clicking a notification (the delegate queues where it came from) brings the
//! window to that host, session, tab and pane at the next poll.

mod m9_support;

use gpui::TestAppContext;
use leon_app::notify;
use m9_support::*;
use serde_json::json;

#[gpui::test]
fn a_clicked_notice_selects_its_pane(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let other = s.app.call("session.create", json!({ "label": "ci" })).unwrap();
    let sid = other["session_id"].as_str().unwrap().to_string();
    let tab = s.app.call("session.new_window", json!({ "session_id": sid, "window_label": "deploy" })).unwrap();
    let block = tab["block_id"].as_str().unwrap().to_string();
    let w0 = s.app.ctx().state().session(&sid).unwrap().windows[0].id.clone();
    s.app.call("session.focus_window", json!({ "session_id": sid, "window_id": w0 })).unwrap();
    cx.run_until_parked();
    assert_ne!(ws.read_with(cx, |ws, _| ws.session_id()), Some(sid.clone()));

    notify::clicked(notify::Click { host_label: "This Mac".into(), session_id: sid.clone(), block_id: block.clone() });
    let there = shows(cx, |cx| ws.read_with(cx, |ws, _| ws.target_block()) == Some(block.clone()));
    assert!(there, "the clicked notice's pane has the focus");
    assert_eq!(ws.read_with(cx, |ws, _| ws.session_id()), Some(sid));
    s.app.stop();
}
