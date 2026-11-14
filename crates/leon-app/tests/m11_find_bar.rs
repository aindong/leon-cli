//! M11 criterion 8: cmd+f's find bar in the pane header: matches across scrollback, stepping,
//! the case and regex toggles, esc.
mod m11_support;

use gpui::TestAppContext;
use m11_support::*;

/// 200 lines of hay, "needle" on lines 20 and 150 (in the scrollback), "Needle" at the end.
const SCRIPT: &str = "stty -echo; i=1; while [ $i -le 200 ]; do \
    if [ $i = 20 ] || [ $i = 150 ]; then echo needle $i; else echo hay $i; fi; i=$((i+1)); done; \
    echo Needle up; sleep 100";

fn status(ws: &gpui::Entity<leon_app::view::Workspace>, cx: &mut gpui::VisualTestContext) -> Option<(usize, usize)> {
    cx.run_until_parked();
    ws.read_with(cx, |ws, _| ws.find_status())
}

fn shows_line(
    ws: &gpui::Entity<leon_app::view::Workspace>,
    cx: &mut gpui::VisualTestContext,
    id: &str,
    l: &str,
) -> bool {
    view_lines(ws, cx, id).iter().any(|v| v == l)
}

#[gpui::test]
fn find_steps_through_matches_in_the_scrollback(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let id = tab_running(&s, &ws, cx, SCRIPT, "Needle up");
    cx.simulate_keystrokes("cmd-f");
    assert!(drawn(cx, &format!("find-bar-{id}")), "the bar is in the pane's header");
    cx.simulate_keystrokes("n e e d l e");
    assert_eq!(status(&ws, cx), Some((3, 3)), "any case by default; the newest match first");
    assert!(ws.read_with(cx, |ws, _| ws.highlighted_matches(&id)) >= 1, "the match on screen is highlighted");

    cx.simulate_keystrokes("shift-enter");
    assert_eq!(status(&ws, cx), Some((2, 3)));
    assert!(shows_line(&ws, cx, &id, "needle 150"), "the view scrolled to it");
    cx.simulate_keystrokes("shift-enter");
    assert_eq!(status(&ws, cx), Some((1, 3)));
    assert!(shows_line(&ws, cx, &id, "needle 20"));
    assert!(ws.read_with(cx, |ws, _| ws.highlighted_matches(&id)) >= 1, "highlighted in the scrollback too");
    cx.simulate_keystrokes("enter");
    assert_eq!(status(&ws, cx), Some((2, 3)), "enter steps forward");

    cx.simulate_keystrokes("alt-c");
    assert_eq!(status(&ws, cx).map(|s| s.1), Some(2), "case-sensitive: not Needle");
    for _ in 0.."needle".len() {
        cx.simulate_keystrokes("backspace");
    }
    cx.simulate_keystrokes("h . y space 1");
    assert_eq!(status(&ws, cx).map(|s| s.1), Some(0), "a literal dot");
    cx.simulate_keystrokes("alt-r");
    assert!(status(&ws, cx).is_some_and(|s| s.1 > 100), "as a regex: {:?}", status(&ws, cx));

    cx.simulate_keystrokes("escape");
    assert!(!drawn(cx, &format!("find-bar-{id}")), "esc closes the bar");
    assert_eq!(status(&ws, cx), None);
    assert_eq!(ws.read_with(cx, |ws, _| ws.highlighted_matches(&id)), 0, "and the highlights go");
    s.app.stop();
}
