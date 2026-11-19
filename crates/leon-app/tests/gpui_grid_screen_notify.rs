//! ADR R4 (docs/architecture/gpui.md), phase 2: a pushed screen notifies its pane's grid, not the
//! whole workspace (its model event reaches the grid alone, view_feed.rs); a pane scrolled back is the exception,
//! since the root's "↓ N new lines" pill counts its new lines.
mod grid_support;

use std::time::Duration;

use gpui::TestAppContext;
use grid_support::*;

/// One process printing, so the pane's title (its program) stays put.
const BUSY: &str = "stty -echo; echo busy-ready; exec perl -e '$|=1; while (1) { print time, \"\\n\"; select(undef, undef, undef, 0.02) }'";
const HISTORY: &str = "stty -echo; seq 1 3000; echo seq-done; read a; exec perl -e '$|=1; while (1) { print time, \"\\n\"; select(undef, undef, undef, 0.05) }'";

/// Close the session's first tab, a login shell whose prompt sets its title (a sidebar redraw)
/// whenever it gets round to it.
fn close_first_tab(s: &Setup, ws: &gpui::Entity<leon_app::view::Workspace>, cx: &mut gpui::VisualTestContext) {
    let sid = ws.read_with(cx, |ws, _| ws.session_id()).unwrap();
    let (tabs, _) = ws.read_with(cx, |ws, _| ws.windows()).unwrap();
    let first = leaves(&tabs[0]).remove(0);
    s.app.call("block.close", serde_json::json!({ "session_id": sid, "block_id": first })).unwrap();
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.windows()).is_some_and(|(w, _)| w.len() == 1)));
}

#[gpui::test]
fn output_in_a_pane_at_the_bottom_does_not_notify_the_workspace(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let id = tab_running(&s, &ws, cx, BUSY, "busy-ready");
    close_first_tab(&s, &ws, cx);
    // the pane settles first (its fit, its title becoming the program): until the redraws stop
    let redraws = |cx: &mut gpui::VisualTestContext| ws.read_with(cx, |ws, _| ws.root_redraws());
    let settled = (0..20).any(|_| {
        let was = redraws(cx);
        run_for(cx, Duration::from_millis(300));
        redraws(cx) == was
    });
    assert!(settled, "the window kept redrawing");
    let counts = |cx: &mut gpui::VisualTestContext| {
        let (p, r) = ws.read_with(cx, |ws, _| (ws.prepares(), ws.root_redraws()));
        (p, r, grid_renders(&ws, cx, &id))
    };
    let before = counts(cx);
    run_for(cx, Duration::from_secs(1));
    let after = counts(cx);
    s.app.stop();
    assert!(after.2 - before.2 >= 3, "the grid drew the output: {before:?} → {after:?}");
    assert_eq!((after.0, after.1), (before.0, before.1), "the workspace was notified: {before:?} → {after:?}");
}

#[gpui::test]
fn output_in_a_pane_scrolled_back_redraws_its_new_lines_pill(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let id = tab_running(&s, &ws, cx, HISTORY, "seq-done");
    let t = term(&s.app, &id);
    assert!(shows(cx, |cx| stays(cx, &t, t.size())), "the pane keeps its size");
    act(&ws, cx, "client.scroll.top");
    assert!(ws.read_with(cx, |ws, _| ws.view_top(&id)).is_some(), "scrolled back");
    t.write(b"\n").unwrap();
    let before = ws.read_with(cx, |ws, _| ws.prepares());
    let pill = format!("jump-{id}");
    let counted = shows(cx, |cx| drawn(cx, &pill));
    let after = ws.read_with(cx, |ws, _| ws.prepares());
    s.app.stop();
    assert!(counted, "the pill shows the new lines");
    assert!(after > before, "the workspace was told of the scrolled pane's new lines");
}
