//! Loose ends criterion 5 (.agent/plans/2026-11-15-loose-ends-m12-m21.md): ⌘↩ in the switcher
//! puts a pane of another session on this machine in a split beside the focused one (tmux's
//! join-pane); on a row of another machine it says why it can't, and nothing moves.

mod m9_support;

use gpui::TestAppContext;
use m9_support::*;
use serde_json::json;

#[gpui::test]
fn cmd_enter_joins_a_pane_of_another_session_and_explains_another_machine(cx: &mut TestAppContext) {
    // the hosts list goes in a private state dir, and ssh to the added host fails at once: set
    // before the server starts (the only test in this binary)
    let state =
        tempfile::Builder::new().permissions(std::os::unix::fs::PermissionsExt::from_mode(0o700)).tempdir().unwrap();
    std::env::set_var("LEON_STATE_DIR", state.path());
    std::env::set_var("LEON_SSH", "false");
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let here = ws.read_with(cx, |ws, _| ws.session_id()).unwrap();
    let w0 = s.app.ctx().state().session(&here).unwrap().windows[0].id.clone();
    let other = s.app.call("session.create", json!({ "label": "deploys" })).unwrap();
    let other_sid = other["session_id"].as_str().unwrap().to_string();
    let w = s.app.call("session.new_window", json!({ "session_id": other_sid, "window_label": "rollout" })).unwrap();
    let moved = w["block_id"].as_str().unwrap().to_string();
    cx.run_until_parked();

    cx.simulate_keystrokes("cmd-k");
    cx.simulate_input("rollout");
    // the switcher lists the app's copy of the model, which hears of the new tab by events
    let first = |cx: &mut gpui::VisualTestContext| ws.read_with(cx, |ws, _| ws.switcher_rows()).first().cloned();
    assert!(
        shows(cx, |cx| first(cx).as_deref() == Some("This Mac › deploys › rollout")),
        "the app lists the other session's tab first: {:?}",
        ws.read_with(cx, |ws, _| ws.switcher_rows())
    );
    cx.simulate_keystrokes("cmd-enter");
    cx.run_until_parked();
    assert!(!ws.read_with(cx, |ws, _| ws.switcher_open()));
    assert!(
        eventually(|| {
            let st = s.app.ctx().state();
            let sess = st.session(&here).unwrap();
            let w = &sess.windows[sess.active];
            w.id == w0 && w.layout.as_ref().is_some_and(|l| l.leaves().contains(&moved.as_str()))
        }),
        "the pane of the other session sits beside the focused one in this window"
    );
    {
        let st = s.app.ctx().state();
        let deploys = st.session(&other_sid).unwrap();
        assert!(deploys.blocks.iter().all(|b| b.id != moved), "it left the other session");
        assert_eq!(deploys.windows.len(), 1, "its emptied tab closed");
    }
    assert_eq!(ws.read_with(cx, |ws, _| ws.session_id()), Some(here.clone()), "this session is still shown");

    // a host whose ssh fails: its rows can't join a window of this machine
    cx.simulate_keystrokes("cmd-shift-p");
    cx.simulate_input("Add Host");
    cx.simulate_keystrokes("enter");
    cx.simulate_input("devbox");
    cx.simulate_keystrokes("enter");
    cx.simulate_input("ssh://dev@devbox");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert!(ws.read_with(cx, |ws, _| ws.host_rows().len()) == 2, "devbox is listed");
    let blocks = s.app.ctx().state().session(&here).unwrap().blocks.len();
    ws.update_in(cx, |ws, window, cx| {
        ws.run_action(
            "session.switch",
            Some(&json!({ "host": 1, "session_id": "session:x", "split": true, "block_id": "block:x" })),
            "palette",
            window,
            cx,
        );
    });
    cx.run_until_parked();
    assert!(ws.read_with(cx, |ws, _| ws.switcher_open()), "the switcher stays open");
    let note = ws.read_with(cx, |ws, _| ws.switcher_note()).unwrap_or_default();
    assert!(note.contains("devbox") && note.contains("This Mac"), "the note names both machines: {note:?}");
    assert_eq!(ws.read_with(cx, |ws, _| ws.session_id()), Some(here.clone()), "nothing moved");
    assert_eq!(s.app.ctx().state().session(&here).unwrap().blocks.len(), blocks);
    s.app.stop();
}
