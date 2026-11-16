//! ADR R5 (docs/architecture/gpui.md): the model work that `render` did before each frame now runs
//! when the workspace hears of a change. Each test changes one call's input from outside the
//! window, as the CLI does (the app hears of it only through the reader's wake and the poll), and
//! waits for the call's effect without a key or click, which would draw a frame of its own.
//! The other moved calls are covered where they already were: `run_find` by
//! perf_idle_wake::find_runs_again_on_output_that_came_just_after_it, `note_fades` by
//! fade_view::a_focused_float_or_drawer_fades_the_tiles_behind_it, `settle_overview` by
//! overview_motion, and the frame bookkeeping (`drew_frame`) by
//! perf_idle_wake::a_working_indicator_polls_and_redraws_at_the_spinner_pace.
mod m20b_support;

use gpui::TestAppContext;
use leon_core::program_status::State;
use m20b_support::*;
use serde_json::json;

/// `note_focus`: a pane the CLI focuses has seen its done, which the app tells the server.
#[gpui::test]
fn a_pane_focused_from_the_cli_has_its_done_cleared(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    act(&ws, cx, "pane.split.right");
    let sid = sid(&ws, cx);
    let (w, _) = window(&s.app, &ws, cx);
    let [a, b] = <[String; 2]>::try_from(leaves(&w)).unwrap();
    s.app.call("session.focus_block", json!({ "session_id": sid, "block_id": a })).unwrap();
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.target_block()) == Some(a.clone())));
    run_until(&term(&s.app, &b), r"printf '\033]7501;state=done\007'; echo d-sent", "d-sent");
    let states = |id: &str| term(&s.app, id).program_status().into_iter().map(|(_, r)| r.state).collect::<Vec<_>>();
    assert!(shows(cx, |_| states(&b) == vec![State::Done]), "b reports done while a has the focus");

    s.app.call("session.focus_block", json!({ "session_id": sid, "block_id": b })).unwrap();
    let cleared = shows(cx, |_| states(&b).is_empty());
    s.app.stop();
    assert!(cleared, "focusing b from the CLI clears its done");
}

/// `note_slides`: a drawer the CLI hides slides out.
#[gpui::test]
fn a_drawer_hidden_from_the_cli_slides(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.set_reduce_motion(false));
    let sid = sid(&ws, cx);
    let toggle = |create: bool| {
        s.app.call("session.toggle", json!({ "session_id": sid, "label": "scratch", "create": create })).unwrap();
    };
    toggle(true);
    let id = view_window(&s.app, &sid)["docked"][0]["block_id"].as_str().unwrap().to_string();
    let dock = format!("dock-{id}");
    assert!(shows(cx, |cx| drawn(cx, &dock)), "the drawer shows");
    // a slide ends on the poll's own timer, which runs on the test clock: move it with real time
    let rested = |cx: &mut gpui::VisualTestContext| {
        cx.executor().advance_clock(std::time::Duration::from_millis(10));
        !ws.read_with(cx, |ws, _| ws.sliding())
    };
    assert!(shows(cx, rested), "its first slide ends");
    toggle(false);
    let slid = shows(cx, |cx| ws.read_with(cx, |ws, _| ws.sliding()));
    s.app.stop();
    assert!(slid, "hiding it from the CLI starts a slide");
}

/// The find pruning: a find bar whose pane left the screen (another tab selected) closes.
#[gpui::test]
fn a_find_bar_closes_when_the_cli_selects_another_tab(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let sid = sid(&ws, cx);
    cx.simulate_keystrokes("cmd-f");
    assert!(ws.read_with(cx, |ws, _| ws.find_status()).is_some(), "the find bar is open");
    s.app.call("session.new_window", json!({ "session_id": sid })).unwrap();
    let closed = shows(cx, |cx| ws.read_with(cx, |ws, _| ws.find_status()).is_none());
    s.app.stop();
    assert!(closed, "the find bar's pane left the screen, and the bar with it");
}

/// `sync_pane`: a pane scrolled back that is resized from outside starts its numbering over, so it
/// follows the output again.
#[gpui::test]
fn a_scrolled_pane_resized_from_outside_starts_over(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let sid = sid(&ws, cx);
    // a quiet program, not a login shell whose prompt may redraw (and renumber) on its own
    let layout = json!({ "block": { "options": { "command": ["sh", "-c", "seq 1 3000; echo seq-done; sleep 100"] } } });
    let r = s.app.call("session.new_window", json!({ "session_id": sid, "layout": layout })).unwrap();
    let id = r["block_id"].as_str().unwrap().to_string();
    let text = |cx: &mut gpui::VisualTestContext| ws.read_with(cx, |ws, _| ws.view_text(&id)).unwrap_or_default();
    assert!(shows(cx, |cx| text(cx).contains("seq-done")), "the program ran");
    // the app's own fit to the pane renumbers it too: scroll once that size has landed
    let t = term(&s.app, &id);
    assert!(shows(cx, |cx| stays(cx, &t, t.size())), "the pane keeps its size");
    act(&ws, cx, "client.scroll.top");
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.view_top(&id)).is_some()), "scrolled to the top");
    t.resize(10, 40).unwrap();
    let over = shows(cx, |cx| ws.read_with(cx, |ws, _| ws.view_top(&id)).is_none());
    s.app.stop();
    assert!(over, "the new size renumbered the pane, which follows the output again");
}

/// `settle`: the selected session ended from the CLI hands the selection to a live one.
#[gpui::test]
fn the_selected_session_destroyed_from_the_cli_hands_over(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let first = sid(&ws, cx);
    let second = s.app.call("session.create", json!({})).unwrap()["session_id"].as_str().unwrap().to_string();
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.session_count()) == 2));
    s.app.call("session.destroy", json!({ "session_id": first })).unwrap();
    let moved = shows(cx, |cx| ws.read_with(cx, |ws, _| ws.session_id()) == Some(second.clone()));
    s.app.stop();
    assert!(moved, "the selection moved to the live session");
}

/// `note_recent`: tabs the CLI selects are the switcher's most recent, newest first.
#[gpui::test]
fn tabs_selected_from_the_cli_are_the_most_recent(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let sid = sid(&ws, cx);
    let shown = |cx: &mut gpui::VisualTestContext, wid: &str| {
        shows(cx, |cx| ws.read_with(cx, |ws, _| ws.active_window().is_some_and(|w| w.id == wid)))
    };
    let mut tabs = Vec::new();
    for label in ["tab-b", "tab-c"] {
        let r = s.app.call("session.new_window", json!({ "session_id": sid, "window_label": label })).unwrap();
        let wid = r["window_id"].as_str().unwrap().to_string();
        assert!(shown(cx, &wid), "{label} is on screen");
        tabs.push(wid);
    }
    act(&ws, cx, "session.switch");
    let rows = ws.read_with(cx, |ws, _| ws.switcher_rows());
    let at = |label: &str| rows.iter().position(|r| r.contains(label));
    s.app.stop();
    let (c, b) = (at("tab-c").expect("tab-c is listed"), at("tab-b").expect("tab-b is listed"));
    assert!(c < b, "tab-c, then tab-b: {rows:?}");
}

/// `watch_screens`: a tab the CLI makes and selects has its pane's screen pushed.
#[gpui::test]
fn a_tab_made_from_the_cli_is_watched(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let sid = sid(&ws, cx);
    let r = s.app.call("session.new_window", json!({ "session_id": sid })).unwrap();
    let block = r["block_id"].as_str().unwrap().to_string();
    let watched = shows(cx, |cx| ws.read_with(cx, |ws, _| ws.watching()) == vec![block.clone()]);
    let screen = shows(cx, |cx| ws.read_with(cx, |ws, _| ws.screen_text(&block)).is_some());
    s.app.stop();
    assert!(watched && screen, "the new tab's pane is watched and its screen came");
}
