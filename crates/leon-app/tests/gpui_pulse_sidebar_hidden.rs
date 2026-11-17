//! ADR R6 (docs/architecture/gpui.md), phase 3: only a section that draws a working indicator
//! follows the Pulse. In top style the sidebar leaves out the selected session's status (its tabs
//! in the title bar show it), so a working tab there ticks the title bar, not the sidebar.
mod grid_support;

use std::time::Duration;

use gpui::TestAppContext;
use grid_support::*;

#[gpui::test]
fn a_status_the_sidebar_leaves_out_does_not_tick_it(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    act(&ws, cx, "client.tab_style.toggle");
    tab_running(&s, &ws, cx, WORKING, "w-ready");
    assert!(
        shows(cx, |cx| ws.read_with(cx, |ws, cx| ws.pulse_state(cx).0)),
        "the title bar's tab works: the clock runs"
    );
    run_for(cx, Duration::from_millis(500));
    let (sidebar, titlebar) = (section_renders(&ws, cx, "sidebar"), section_renders(&ws, cx, "titlebar"));
    run_for(cx, Duration::from_secs(1));
    let sidebar = section_renders(&ws, cx, "sidebar") - sidebar;
    let titlebar = section_renders(&ws, cx, "titlebar") - titlebar;
    s.app.stop();
    assert!(titlebar >= 4, "the title bar's spinner moved {titlebar} times in 1 s");
    assert!(sidebar <= 1, "the sidebar, which shows no working status, rendered {sidebar} times in 1 s");
}
