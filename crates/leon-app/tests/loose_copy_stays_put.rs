//! Loose ends criterion 3 (.agent/plans/2026-11-15-loose-ends-m12-m21.md): copy mode holds the
//! view still while output arrives, with its cursor on the bottom row too (tmux); leaving it
//! follows the output again.
mod m11_support;

use gpui::TestAppContext;
use m11_support::*;

/// A line every 50 ms, after enough to fill the scrollback a little.
const SCRIPT: &str = "stty -echo; seq 1 100; i=0; while :; do i=$((i+1)); echo tick $i; sleep 0.05; done";

/// The rows the pane shows but its last: the program's cursor row can still be written in place.
fn view(ws: &gpui::Entity<leon_app::view::Workspace>, cx: &mut gpui::VisualTestContext, id: &str) -> Vec<String> {
    cx.run_until_parked();
    let mut rows = view_lines(ws, cx, id);
    rows.pop();
    rows
}

/// Wait until `n` more ticks have reached the app's copy of the live screen.
fn ticks_pass(ws: &gpui::Entity<leon_app::view::Workspace>, cx: &mut gpui::VisualTestContext, id: &str, n: usize) {
    let last = |cx: &mut gpui::VisualTestContext| {
        let text = ws.read_with(cx, |ws, _| ws.screen_text(id)).unwrap_or_default();
        text.lines().filter_map(|l| l.strip_prefix("tick ")?.trim().parse::<usize>().ok()).max().unwrap_or(0)
    };
    let from = last(cx);
    assert!(shows(cx, |cx| last(cx) >= from + n), "the program stopped printing");
}

#[gpui::test]
fn copy_mode_keeps_the_view_still_at_the_bottom(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let id = tab_running(&s, &ws, cx, SCRIPT, "tick 3");

    cx.simulate_keystrokes("cmd-shift-c");
    let before = view(&ws, cx, &id);
    ticks_pass(&ws, cx, &id, 5);
    assert_eq!(view(&ws, cx, &id), before, "entered at the bottom, the view stays put");
    assert!(ws.read_with(cx, |ws, _| ws.view_top(&id)).is_some());

    // up a line and back down to the bottom row
    cx.simulate_keystrokes("k j");
    let before = view(&ws, cx, &id);
    ticks_pass(&ws, cx, &id, 5);
    assert_eq!(view(&ws, cx, &id), before, "back on the bottom row, the view still stays put");

    cx.simulate_keystrokes("shift-g");
    let before = view(&ws, cx, &id);
    ticks_pass(&ws, cx, &id, 5);
    assert_eq!(view(&ws, cx, &id), before, "G goes to the last line and stays there");

    // a shorter screen drops the blank rows under the cursor: copy mode stays, on the new last
    // line, and the view holds still from the next screen on
    let rows = view_lines(&ws, cx, &id).len();
    cx.simulate_resize(gpui::size(gpui::px(1100.), gpui::px(700.)));
    assert!(shows(cx, |cx| view_lines(&ws, cx, &id).len() != rows), "the pane took the new size");
    ticks_pass(&ws, cx, &id, 1);
    let top = ws.read_with(cx, |ws, _| ws.view_top(&id));
    assert!(top.is_some() && ws.read_with(cx, |ws, _| ws.copy_cursor(&id)).is_some(), "still in copy mode, held");
    ticks_pass(&ws, cx, &id, 5);
    assert_eq!(ws.read_with(cx, |ws, _| ws.view_top(&id)), top, "after a resize too");

    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert_eq!(ws.read_with(cx, |ws, _| ws.view_top(&id)), None, "leaving copy mode follows the output");
    s.app.stop();
}
