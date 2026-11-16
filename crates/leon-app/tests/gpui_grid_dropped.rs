//! ADR R10 (docs/architecture/gpui.md), phase 2: a pane's grid view lives while the pane is shown;
//! one that left the screen is dropped (and its drawn record with it), not kept alive unseen.
mod grid_support;

use gpui::TestAppContext;
use grid_support::*;
use serde_json::json;

#[gpui::test]
fn a_pane_that_leaves_the_screen_drops_its_grid(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let first = tab_running(&s, &ws, cx, "stty -echo; echo first-ready; sleep 300", "first-ready");
    let second = split_running(&s, &ws, cx, &first, "stty -echo; echo second-ready; sleep 300", "second-ready");
    let has = |cx: &mut gpui::VisualTestContext, id: &str| ws.read_with(cx, |ws, _| ws.grid_drew(id).is_some());
    assert!(has(cx, &first) && has(cx, &second));

    // another tab: both leave the screen
    let sid = ws.read_with(cx, |ws, _| ws.session_id()).unwrap();
    let third = tab_running(&s, &ws, cx, "stty -echo; echo third-ready; sleep 300", "third-ready");
    let gone = shows(cx, |cx| !has(cx, &first) && !has(cx, &second));
    let kept = has(cx, &third);
    // closed: its grid goes too
    s.app.call("block.close", json!({ "session_id": sid, "block_id": third })).unwrap();
    let closed = shows(cx, |cx| !has(cx, &third));
    s.app.stop();
    assert!(gone, "the hidden tab's grids were dropped");
    assert!(kept, "the shown pane has its grid");
    assert!(closed, "a closed pane's grid was dropped");
}
