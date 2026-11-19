//! Review proof (security, cross-model): text composed on one pane is never written to another
//! pane that became the target before the commit.
mod focus_support;

use focus_support::*;
use gpui::{InputHandler, TestAppContext};
use leon_app::view::PaneInput;
use serde_json::json;

#[gpui::test]
fn a_commit_goes_only_to_the_pane_it_was_composed_on(cx: &mut TestAppContext) {
    let s = plain_setup();
    let (ws, cx) = open(cx, &s.app);
    act(&ws, cx, "pane.split.right");
    let sid = ws.read_with(cx, |ws, _| ws.session_id()).unwrap();
    let (w, _) = window(&s.app, &ws, cx);
    let [a, b] = <[String; 2]>::try_from(leaves(&w)).unwrap();
    let target = |cx: &mut gpui::VisualTestContext| ws.read_with(cx, |ws, _| ws.target_block());
    s.app.call("session.focus_block", json!({ "session_id": sid, "block_id": a })).unwrap();
    assert!(shows(cx, |cx| target(cx) == Some(a.clone())), "a is the target");
    settle(&term(&s.app, &a));
    settle(&term(&s.app, &b));
    let mut h = PaneInput::new(ws.clone());
    cx.update(|window, cx| h.replace_and_mark_text_in_range(None, "にほ", None, window, cx));
    cx.run_until_parked();
    // another client moves the focus while the composition is open
    s.app.call("session.focus_block", json!({ "session_id": sid, "block_id": b })).unwrap();
    assert!(shows(cx, |cx| target(cx) == Some(b.clone())), "b is the target");
    cx.update(|window, cx| h.replace_text_in_range(None, "imecommitxyz", window, cx));
    cx.run_until_parked();
    let leaked = eventually(|| term(&s.app, &b).text(false).contains("imecommitxyz"));
    s.app.stop();
    assert!(!leaked, "SEC-LEAK text composed on pane a was written to pane b");
}
