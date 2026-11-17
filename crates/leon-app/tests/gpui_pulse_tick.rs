//! ADR R3/R6 (docs/architecture/gpui.md), phase 3: one Pulse clocks every spinner, and a tick
//! re-renders only the sections showing a working indicator, and the root, never a grid.
mod grid_support;

use std::time::Duration;

use gpui::TestAppContext;
use grid_support::*;

#[gpui::test]
fn a_spinner_tick_renders_its_sections_and_the_root_but_no_grid(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let calm = tab_running(&s, &ws, cx, &quiet("calm-ready"), "calm-ready");
    let busy = split_running(&s, &ws, cx, &calm, WORKING, "w-ready");
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, cx| ws.pulse_state(cx).0)), "the Pulse runs");
    run_for(cx, Duration::from_millis(300));
    let (hb, hc) = (format!("header-{busy}"), format!("header-{calm}"));
    let counts = |cx: &mut gpui::VisualTestContext| {
        let renders = ws.read_with(cx, |ws, _| ws.renders());
        [
            section_renders(&ws, cx, &hb),
            section_renders(&ws, cx, "sidebar"),
            renders,
            section_renders(&ws, cx, &hc),
            grid_renders(&ws, cx, &busy),
            grid_renders(&ws, cx, &calm),
        ]
    };
    let before = counts(cx);
    run_for(cx, Duration::from_secs(1));
    let after = counts(cx);
    s.app.stop();
    let d: Vec<u64> = after.iter().zip(before).map(|(a, b)| a - b).collect();
    assert!(d[0] >= 5 && d[1] >= 5 && d[2] >= 5, "the working header, the sidebar and the root ticked: {d:?}");
    assert_eq!(&d[3..], &[0, 0, 0], "the quiet header and both grids stayed cached: {d:?}");
}
