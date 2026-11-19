//! Motion PR 3 (ADR R3): an overlay's fade re-renders only what draws it: the palette's no grid and
//! no section (past the focus moving in and out); the find bar's only its pane's header.
mod layout_support;

use std::collections::HashMap;

use gpui::TestAppContext;
use layout_support::*;

#[gpui::test]
fn a_fade_renders_only_what_draws_it(cx: &mut TestAppContext) {
    let s = plain_setup(); // a login shell's prompt may set its tab's title at any time
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.set_reduce_motion(false));
    let a = tab_running(&s, &ws, cx, &quiet("a-ready"), "a-ready");
    // the server looks at each pane's program every 200 ms, so a change (sh, stty, then sleep here)
    // can reach the app after the marker and render the sidebar and the header: count once the app
    // knows every pane's program
    let programs = |cx: &mut gpui::VisualTestContext| {
        let remote = ws.read_with(cx, |ws, _| ws.host_remote(leon_app::hosts::THIS_MAC)).unwrap();
        let model = remote.model();
        model.sessions.iter().flat_map(|s| &s.blocks).map(|b| (b.id.clone(), b.foreground.clone())).collect::<Vec<_>>()
    };
    let known = |p: &[(String, Option<Option<String>>)]| {
        p.iter().all(
            |(id, f)| if *id == a { f.as_ref() == Some(&Some("sleep".into())) } else { matches!(f, Some(Some(_))) },
        )
    };
    assert!(shows(cx, |cx| known(&programs(cx))), "the app knows each pane's program: {:?}", programs(cx));
    cx.run_until_parked();
    to_rest(&ws, cx);
    let counts = |cx: &mut gpui::VisualTestContext| -> (u64, HashMap<String, u64>) {
        (grid_renders(&ws, cx, &a), ws.read_with(cx, |ws, _| ws.section_renders()))
    };
    let before = counts(cx);
    act(&ws, cx, "client.palette.toggle");
    let n = to_rest(&ws, cx);
    cx.simulate_keystrokes("escape");
    let m = to_rest(&ws, cx);
    let after = counts(cx);
    act(&ws, cx, "client.find.open");
    let (_, sections0) = counts(cx);
    let k = to_rest(&ws, cx);
    let (grid2, sections2) = counts(cx);
    s.app.stop();
    assert!(n >= 2 && m >= 1, "the palette faded in ({n} frames) and out ({m})");
    // the palette takes the focus and gives it back (GPUI phase 6): each focus change refreshes
    // the window once (GPUI drops every cache for that frame, ADR R3); the fade's own frames
    // render no grid and no section
    let refreshed = |c: u64| c + 2;
    let sections: HashMap<String, u64> = before.1.iter().map(|(k, v)| (k.clone(), refreshed(*v))).collect();
    assert_eq!(after, (refreshed(before.0), sections), "the palette's fade rendered no grid and no section");
    let header = format!("header-{a}");
    let moved: Vec<_> =
        sections2.iter().filter(|(k, v)| sections0.get(*k) != Some(v)).map(|(k, _)| k.clone()).collect();
    assert!(k >= 2, "the find bar faded in ({k} frames)");
    assert_eq!(moved, vec![header], "only its pane's header rendered as it faded");
    assert_eq!(grid2, after.0 + 1, "and no grid, past the focus moving to the bar");
}
