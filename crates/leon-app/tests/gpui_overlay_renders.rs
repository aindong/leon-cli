//! Motion PR 3 (ADR R3): an overlay's fade re-renders only what draws it: the palette's no grid and
//! no section; the find bar's only its pane's header.
mod layout_support;

use std::collections::HashMap;

use gpui::TestAppContext;
use layout_support::*;

#[gpui::test]
fn a_fade_renders_only_what_draws_it(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.set_reduce_motion(false));
    let a = tab_running(&s, &ws, cx, &quiet("a-ready"), "a-ready");
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
    assert_eq!(after, before, "the palette's fade rendered no grid and no section");
    let header = format!("header-{a}");
    let moved: Vec<_> =
        sections2.iter().filter(|(k, v)| sections0.get(*k) != Some(v)).map(|(k, _)| k.clone()).collect();
    assert!(k >= 2, "the find bar faded in ({k} frames)");
    assert_eq!(moved, vec![header], "only its pane's header rendered as it faded");
    assert_eq!(grid2, after.0, "and no grid");
}
