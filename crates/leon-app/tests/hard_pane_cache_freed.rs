//! Audit A4 (docs/audit-0.1.0-app.md): a pane's scroll state and history rows go when the pane
//! does (the host-gone half is in crates/leon/tests/hard_remote_copy.rs).
mod m11_support;

use gpui::TestAppContext;
use m11_support::*;

#[gpui::test]
fn a_closed_pane_leaves_no_history_rows_behind(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let id = tab_running(&s, &ws, cx, "seq 1 500; echo ready; sleep 300", "ready");
    // paged up again until the pane has history to show (its first resize can come after `ready`)
    let fetched = shows(cx, |cx| {
        act(&ws, cx, "client.scroll.page_up");
        ws.read_with(cx, |ws, _| ws.cached_rows(&id)).is_some_and(|n| n > 0)
    });
    assert!(fetched, "rows fetched: {:?}", ws.read_with(cx, |ws, _| ws.cached_rows(&id)));
    act(&ws, cx, "pane.close");
    let gone = shows(cx, |cx| ws.read_with(cx, |ws, _| ws.cached_rows(&id)).is_none());
    assert!(gone, "{:?} history rows kept for a closed pane", ws.read_with(cx, |ws, _| ws.cached_rows(&id)));
    s.app.stop();
}
