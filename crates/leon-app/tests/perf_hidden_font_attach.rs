//! `leon attach` sizes the panes it shows to its own terminal: the app's font no longer changes
//! those the app doesn't show.
mod m9_support;
use gpui::TestAppContext;
use m9_support::*;
use serde_json::json;

#[gpui::test]
fn a_pane_another_client_sized_keeps_that_size(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let shown = focused(&s.app, &ws, cx);
    let (sid, block) = hidden_session(&s.app, &ws, cx);
    let hidden = term(&s.app, &block);
    let args = json!({ "rows": 30, "cols": 100 });
    let resize = json!({ "session_id": sid, "block_id": block, "method": "dev.leon.terminal.resize", "args": args });
    s.app.call("block.method", resize).unwrap();
    assert_eq!(hidden.size(), (30, 100), "the other client's size");
    let before = shown.size();

    cx.simulate_keystrokes("cmd-=");
    assert!(shows(cx, |_| shown.size() != before), "the shown pane follows the font");
    let kept = stays(cx, &hidden, (30, 100));
    let got = hidden.size();
    s.app.stop();
    assert!(kept, "the app's font change leaves the other client's size: {got:?}");
}
