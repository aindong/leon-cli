//! A clicked notification wakes every window, not only the one made last: the click queue is
//! the process's, and the window that knows the pane may be an older one. In a test nothing else
//! would wake it (the test clock never fires the poll's timer), which is how
//! `loose_switcher_edges` lost its click on the macOS runner when another test's window was made
//! after its own.

mod m9_support;

use gpui::TestAppContext;
use leon_app::notify;
use m9_support::*;
use serde_json::json;

#[gpui::test]
fn a_click_reaches_the_window_that_knows_it_though_another_was_made_later(
    cx: &mut TestAppContext,
    later: &mut TestAppContext,
) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let other = s.app.call("session.create", json!({ "label": "ci" })).unwrap();
    let sid = other["session_id"].as_str().unwrap().to_string();
    let tab = s.app.call("session.new_window", json!({ "session_id": sid, "window_label": "deploy" })).unwrap();
    let block = tab["block_id"].as_str().unwrap().to_string();
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.pane_title(&block).is_some())), "the app heard of the tab");

    // a second window, on a server of its own, made after the first
    let s2 = setup();
    let (ws2, later) = open(later, &s2.app);
    let there = ws2.read_with(later, |ws, _| ws.target_block());

    // nothing else may wake the first window: every shell has drawn its prompt, and what that
    // woke has run
    let here = ws.read_with(cx, |ws, _| ws.session_id()).unwrap();
    for session in [&here, &sid] {
        let view = s.app.call("session.view", json!({ "session_id": session })).unwrap();
        for id in view["windows"].as_array().unwrap().iter().flat_map(|w| w["block_ids"].as_array().unwrap()) {
            settle(&term(&s.app, id.as_str().unwrap()));
        }
    }
    cx.run_until_parked();
    assert_ne!(ws.read_with(cx, |ws, _| ws.target_block()), Some(block.clone()));

    notify::clicked(notify::Click { host_label: "This Mac".into(), session_id: sid.clone(), block_id: block.clone() });
    assert!(
        shows(cx, |cx| ws.read_with(cx, |ws, _| ws.target_block()) == Some(block.clone())),
        "one click took the older window to its pane"
    );
    assert_eq!(ws.read_with(cx, |ws, _| ws.session_id()), Some(sid));
    assert_eq!(ws2.read_with(later, |ws, _| ws.target_block()), there, "the other window stayed where it was");
    s.app.stop();
    s2.app.stop();
}
