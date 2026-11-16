//! ADR R3 (docs/architecture/gpui.md), phase 2: a cached grid skips its prepaint, where the PTY is
//! fitted to it, unless notified; a font change is an input, so the PTY follows it.
mod grid_support;

use gpui::TestAppContext;
use grid_support::*;

#[gpui::test]
fn a_font_change_resizes_the_pty(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let id = tab_running(&s, &ws, cx, "stty -echo; echo ready; sleep 300", "ready");
    let t = term(&s.app, &id);
    assert!(shows(cx, |cx| stays(cx, &t, t.size())), "the pane keeps its size");
    let (rows, cols) = t.size();
    act(&ws, cx, "client.font.increase");
    act(&ws, cx, "client.font.increase");
    let fitted = shows(cx, |_| {
        let (r, c) = t.size();
        r < rows && c < cols
    });
    let now = t.size();
    s.app.stop();
    assert!(fitted, "a larger font fits fewer cells: {rows}x{cols}, then {now:?}");
}
