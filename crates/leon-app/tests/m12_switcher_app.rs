//! M12 criterion 2: ⌘K opens the switcher over the palette's box; typing narrows it, ↩ jumps to
//! the session and tab picked; ⌘↩ on a tab of the same session moves its pane beside the focused one.

mod m9_support;

use gpui::TestAppContext;
use m9_support::*;
use serde_json::json;

#[gpui::test]
fn cmd_k_then_enter_jumps_to_another_sessions_tab(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let first = ws.read_with(cx, |ws, _| ws.session_id()).unwrap();
    let other = s.app.call("session.create", json!({ "label": "deploys" })).unwrap();
    let other_sid = other["session_id"].as_str().unwrap().to_string();
    let w = s.app.call("session.new_window", json!({ "session_id": other_sid, "window_label": "rollout" })).unwrap();
    let rollout = w["window_id"].as_str().unwrap().to_string();
    let w0 = s.app.ctx().state().session(&other_sid).unwrap().windows[0].id.clone();
    s.app.call("session.focus_window", json!({ "session_id": other_sid, "window_id": w0 })).unwrap();
    cx.run_until_parked();
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.tab_titles().len()) == 1), "the first session is shown");
    // the switcher lists the app's copy of the model, which refetches after the server's events
    let has = |cx: &mut gpui::VisualTestContext| {
        let rows = ws.read_with(cx, |ws, _| ws.host_rows());
        rows.iter().flat_map(|h| &h.sessions).any(|x| x.id == other_sid && x.tabs.contains(&"rollout".to_string()))
    };
    assert!(shows(cx, has), "the app's model has the new tab");

    cx.simulate_keystrokes("cmd-k");
    assert!(ws.read_with(cx, |ws, _| ws.switcher_open()), "cmd+k opens the switcher");
    let rows = ws.read_with(cx, |ws, _| ws.switcher_rows());
    assert!(rows.iter().any(|r| r.contains("deploys › rollout")), "{rows:?}");
    assert!(rows.iter().all(|r| r.starts_with("This Mac › ")), "rows name their host: {rows:?}");
    cx.simulate_input("rollout");
    let rows = ws.read_with(cx, |ws, _| ws.switcher_rows());
    assert_eq!(rows.first().map(String::as_str), Some("This Mac › deploys › rollout"), "{rows:?}");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert!(!ws.read_with(cx, |ws, _| ws.switcher_open()), "enter closes it");
    assert_eq!(ws.read_with(cx, |ws, _| ws.session_id()), Some(other_sid.clone()));
    assert!(
        eventually(|| {
            let st = s.app.ctx().state();
            let sess = st.session(&other_sid).unwrap();
            sess.windows[sess.active].id == rollout
        }),
        "the tab picked is the active one"
    );

    // with no query the tab shown now comes first, the one shown before it second
    cx.simulate_keystrokes("cmd-k");
    let rows = ws.read_with(cx, |ws, _| ws.switcher_rows());
    assert_eq!(rows.first().map(String::as_str), Some("This Mac › deploys › rollout"), "{rows:?}");
    assert!(rows.get(1).is_some_and(|r| r.starts_with("This Mac › Session 1 › ")), "{rows:?} (first: {first})");
    cx.simulate_keystrokes("escape");
    assert!(!ws.read_with(cx, |ws, _| ws.switcher_open()));
    s.app.stop();
}

#[gpui::test]
fn cmd_enter_moves_a_tab_of_this_session_into_a_split(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let sid = ws.read_with(cx, |ws, _| ws.session_id()).unwrap();
    let w = s.app.call("session.new_window", json!({ "session_id": sid, "window_label": "tests" })).unwrap();
    let moved = w["block_id"].as_str().unwrap().to_string();
    let w0 = s.app.ctx().state().session(&sid).unwrap().windows[0].id.clone();
    s.app.call("session.focus_window", json!({ "session_id": sid, "window_id": w0 })).unwrap();
    cx.run_until_parked();
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.tab_titles().len()) == 2));

    cx.simulate_keystrokes("cmd-k");
    cx.simulate_input("tests");
    cx.simulate_keystrokes("cmd-enter");
    cx.run_until_parked();
    assert!(!ws.read_with(cx, |ws, _| ws.switcher_open()));
    assert!(
        eventually(|| {
            let st = s.app.ctx().state();
            let sess = st.session(&sid).unwrap();
            let w = &sess.windows[sess.active];
            w.id == w0
                && w.layout.as_ref().is_some_and(|l| l.leaves().contains(&moved.as_str()) && l.leaves().len() == 2)
        }),
        "the picked tab's pane sits beside the focused one in the active tab"
    );
    s.app.stop();
}
