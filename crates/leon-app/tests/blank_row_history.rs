//! Output arriving under a pane scrolled back pushes rows from its screen into the history; the
//! app fetches those (`sync_pane`), so once it has run the view holds still with every row filled
//! (GPUI phase 1, criterion 2d). This checks the settled view, not each drawn frame: a frame drawn
//! before the poll runs can still show the row blank (the ADR's one-frame race, phase 5).
mod m20b_support;

use gpui::TestAppContext;
use m20b_support::*;
use serde_json::json;

#[gpui::test]
fn output_under_a_scrolled_pane_has_its_new_history_rows_fetched(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let sid = sid(&ws, cx);
    let script = "stty -echo; seq 1 3000; read a; seq 3001 3100; sleep 100";
    let layout = json!({ "block": { "options": { "command": ["sh", "-c", script] } } });
    let r = s.app.call("session.new_window", json!({ "session_id": sid, "layout": layout })).unwrap();
    let id = r["block_id"].as_str().unwrap().to_string();
    let lines = |cx: &mut gpui::VisualTestContext| {
        let text = ws.read_with(cx, |ws, _| ws.view_text(&id)).unwrap_or_default();
        text.lines().map(|l| l.trim_end().to_string()).collect::<Vec<_>>()
    };
    assert!(shows(cx, |cx| lines(cx).iter().any(|l| l == "3000")), "the program ran");
    let t = term(&s.app, &id);
    assert!(shows(cx, |cx| stays(cx, &t, t.size())), "the pane keeps its size");
    act(&ws, cx, "client.scroll.page_up");
    let before = lines(cx);
    // `seq` fills every row, and page_up (a page less one line) leaves the screen's first row last
    assert!(before.iter().all(|l| !l.is_empty()), "every row is filled before the output: {before:?}");
    assert!(ws.read_with(cx, |ws, _| ws.view_top(&id)).is_some(), "scrolled back");
    let fetches = ws.read_with(cx, |ws, _| ws.fetches());
    let scrolled = t.scrolled();
    t.write(b"\n").unwrap();
    // the 100 lines can push fewer rows than that: a pane grown after its history moved to the
    // store (a pause, scrollback.md) has blank rows under the cursor, as alacritty can't pull
    // stored rows back down. So the count to see is the core's.
    let pushed = || t.scrolled() - scrolled;
    // the model is the reader thread's: wait for the app to have drawn what it read, not only for
    // the model to have it, or the view is read before the poll ran (the ADR's one-frame race)
    let moved = shows(cx, |cx| {
        cx.run_until_parked();
        t.text(true).ends_with("3100")
            && pushed() > 0
            && ws.read_with(cx, |ws, _| ws.new_lines(&id)) == Some(pushed())
            && ws.read_with(cx, |ws, _| ws.fetches()) > fetches
    });
    cx.run_until_parked();
    let after = lines(cx);
    let (counted, now) = (ws.read_with(cx, |ws, _| ws.new_lines(&id)), ws.read_with(cx, |ws, _| ws.fetches()));
    s.app.stop();
    assert!(
        moved,
        "the new lines were counted and the rows they pushed into history were fetched: \
         counted {counted:?} of {} pushed, fetches {fetches} -> {now}",
        pushed()
    );
    assert_eq!(after, before, "the view held still, every row filled");
}
