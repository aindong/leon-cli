//! GPUI phase 5: a storm's held notices come later as one summary, on a timer of its own. With
//! nothing else waking the window, GPUI's clock passing the quiet minute posts it.
mod m20b_support;

use std::time::Duration;

use gpui::TestAppContext;
use m20b_support::*;
use serde_json::json;

#[gpui::test]
fn a_storms_summary_posts_on_its_own_timer(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let sid = sid(&ws, cx);
    let first = ws.read_with(cx, |ws, _| ws.target_block()).unwrap();
    let mut blocks = vec![first.clone()];
    for _ in 0..5 {
        let p = json!({ "session_id": sid, "anchor_id": first, "direction": "vertical", "side": "after",
                        "layout": { "block": {} } });
        let made = s.app.call("session.new_split", p).unwrap();
        blocks.push(made["block_id"].as_str().unwrap().to_string());
    }
    cx.deactivate_window(); // the user is elsewhere: blocked panes notify
    cx.run_until_parked();
    for b in &blocks {
        let p = json!({ "session_id": sid, "block_id": b, "method": "dev.leon.terminal.report_status",
                        "args": { "state": "blocked", "kind": "question" } });
        s.app.call("block.method", p).unwrap();
    }
    let posted = |cx: &mut gpui::VisualTestContext| ws.read_with(cx, |ws, _| ws.notices_posted());
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.activity_rows().len()) == 6), "six blocks need the user");
    assert!(shows(cx, |cx| posted(cx).len() == 4), "a burst of four: {:?}", posted(cx));
    for _ in 0..62 {
        cx.executor().advance_clock(Duration::from_secs(1));
        cx.run_until_parked();
    }
    let all = posted(cx);
    s.app.stop();
    let summary = all.get(4).map(|n| n.title.clone());
    assert_eq!(summary.as_deref(), Some("2 more blocks need you"), "the summary came: {all:?}");
}
