//! App update criterion 9 (.agent/plans/2026-11-16-app-update-notice.md): Restart after an update
//! relaunches the app as Start It Again does and leaves the server alone, so the sessions and their
//! panes are there when the next app attaches.

mod m10b_support;

use gpui::{Modifiers, TestAppContext};
use leon_app::updates::Phase;
use m10b_support::*;
use serde_json::{json, Value};

/// Every session's windows and their panes, as the server has them.
fn layout(s: &Setup) -> Vec<Value> {
    let list = s.app.call("session.list", json!({})).unwrap();
    let mut all: Vec<Value> = list["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| s.app.call("session.view", json!({ "session_id": x["session_id"] })).unwrap())
        .collect();
    all.sort_by_key(|v| v.to_string());
    all
}

#[gpui::test]
fn restart_relaunches_and_keeps_the_sessions_and_panes(cx: &mut TestAppContext) {
    let s = setup();
    {
        let (ws, vcx) = open(cx, &s.app);
        act(&ws, vcx, "pane.split.right");
        s.app.call("session.create", json!({})).unwrap();
        assert!(shows(vcx, |_| layout(&s).len() == 2), "two sessions");
        let before = layout(&s);
        ws.read_with(vcx, |ws, _| ws.updater().set_phase(Phase::Done { version: "9.9.9".into() }));
        let restart = at(vcx, "update-restart").expect("the done notice has Restart");
        vcx.simulate_click(center(restart), Modifiers::none());
        let argv = ws.read_with(vcx, |ws, _| ws.relaunched()).expect("Restart relaunched the app");
        let mut want = vec![std::env::current_exe().unwrap().display().to_string()];
        want.extend(std::env::args().skip(1));
        assert_eq!(argv, want, "this executable (the new one, at the same path) with its arguments");
        assert!(!s.app.stopped(), "Restart never stops the server");
        // the app goes: its window closes, as quitting closes it
        drop(ws);
        vcx.update(|window, _| window.remove_window());
        vcx.run_until_parked();
        assert!(!s.app.stopped(), "nor does the app's going");
        assert_eq!(layout(&s), before, "the server kept every session and pane");
    }
    // the relaunched app attaches to the same server and finds them
    let (ws, vcx) = open(cx, &s.app);
    assert!(shows(vcx, |vcx| ws.read_with(vcx, |ws, _| ws.target_block().is_some())), "the new app shows a pane");
    let rows = ws.read_with(vcx, |ws, _| ws.session_count());
    assert_eq!(rows, 2, "both sessions are in the new app's sidebar");
    s.app.stop();
}
