//! Loose ends criterion 5, the review's edges: ⌘↩ with a floating pane focused still joins the
//! pane (beside a tiled one), a zoomed window unzooms to show it, the can't-split note keeps the
//! row it is about and goes when the list changes, and a notice from before a join goes where the
//! pane is now.

mod m9_support;

use gpui::TestAppContext;
use leon_app::notify;
use m9_support::*;
use serde_json::json;

fn other_tab(s: &Setup) -> (String, String) {
    let other = s.app.call("session.create", json!({ "label": "deploys" })).unwrap();
    let sid = other["session_id"].as_str().unwrap().to_string();
    let w = s.app.call("session.new_window", json!({ "session_id": sid, "window_label": "rollout" })).unwrap();
    (sid, w["block_id"].as_str().unwrap().to_string())
}

fn joined(s: &Setup, here: &str, moved: &str) -> bool {
    eventually(|| s.app.ctx().state().block(moved).is_ok_and(|(sess, _)| sess.id == here))
}

#[gpui::test]
fn cmd_enter_with_a_floating_pane_focused_joins_beside_a_tiled_one(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let here = ws.read_with(cx, |ws, _| ws.session_id()).unwrap();
    let fl = s.app.call("session.new_floating", json!({ "session_id": here })).unwrap();
    let floating = fl["block_id"].as_str().unwrap().to_string();
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.target_block()) == Some(floating.clone())));
    let (_, moved) = other_tab(&s);
    cx.run_until_parked();
    cx.simulate_keystrokes("cmd-k");
    cx.simulate_input("rollout");
    cx.simulate_keystrokes("cmd-enter");
    cx.run_until_parked();
    assert!(joined(&s, &here, &moved), "the pane joined this session though a floating pane had the focus");
    s.app.stop();
}

#[gpui::test]
fn a_pane_that_joins_a_zoomed_window_is_not_hidden(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let here = ws.read_with(cx, |ws, _| ws.session_id()).unwrap();
    let a = ws.read_with(cx, |ws, _| ws.target_block()).unwrap();
    s.app.call("session.zoom_block", json!({ "session_id": here, "block_id": a, "zoom": true })).unwrap();
    let (_, moved) = other_tab(&s);
    cx.run_until_parked();
    cx.simulate_keystrokes("cmd-k");
    cx.simulate_input("rollout");
    cx.simulate_keystrokes("cmd-enter");
    cx.run_until_parked();
    assert!(joined(&s, &here, &moved));
    let st = s.app.ctx().state();
    let sess = st.session(&here).unwrap();
    assert_eq!(sess.windows[sess.active].zoomed, None, "the window unzoomed to show the pane that joined");
    drop(st);
    s.app.stop();
}

#[gpui::test]
fn the_cant_split_note_keeps_its_row_and_goes_when_the_list_changes(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    other_tab(&s);
    cx.run_until_parked();
    cx.simulate_keystrokes("cmd-k");
    cx.simulate_input("rollout");
    cx.run_until_parked();
    let before = ws.read_with(cx, |ws, _| ws.switcher_rows());
    let remote = json!({ "host": 1, "session_id": "session:x", "split": true, "block_id": "block:x" });
    ws.update_in(cx, |ws, window, cx| ws.run_action("session.switch", Some(&remote), "palette", window, cx));
    cx.run_until_parked();
    assert!(ws.read_with(cx, |ws, _| ws.switcher_note()).is_some());
    assert_eq!(ws.read_with(cx, |ws, _| ws.switcher_rows()), before, "the note kept the filtered rows");
    cx.simulate_input("x");
    assert_eq!(ws.read_with(cx, |ws, _| ws.switcher_note()), None, "typing clears the note");

    ws.update_in(cx, |ws, window, cx| ws.run_action("session.switch", Some(&remote), "palette", window, cx));
    cx.simulate_keystrokes("cmd-shift-p");
    cx.simulate_keystrokes("cmd-k");
    cx.run_until_parked();
    assert!(ws.read_with(cx, |ws, _| ws.switcher_open()));
    assert_eq!(ws.read_with(cx, |ws, _| ws.switcher_note()), None, "a fresh list has no old note");
    s.app.stop();
}

#[gpui::test]
fn a_notice_from_before_a_join_goes_where_the_pane_is_now(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let here = ws.read_with(cx, |ws, _| ws.session_id()).unwrap();
    let anchor = ws.read_with(cx, |ws, _| ws.target_block()).unwrap();
    let (sid, block) = other_tab(&s);
    s.app
        .call("session.move_block", json!({ "session_id": here, "block_id": block, "anchor_block_id": anchor }))
        .unwrap();
    let in_view = |ws: &leon_app::view::Workspace| {
        ws.active_window().and_then(|w| w.layout).is_some_and(|l| l.leaves().contains(&block.as_str()))
    };
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| in_view(ws))), "the app saw the join");
    // posted while the block was still in `deploys`
    notify::clicked(notify::Click { host_label: "This Mac".into(), session_id: sid, block_id: block.clone() });
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.target_block()) == Some(block.clone())));
    assert_eq!(ws.read_with(cx, |ws, _| ws.session_id()), Some(here));
    s.app.stop();
}
