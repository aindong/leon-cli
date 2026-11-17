//! ADR R3 (docs/architecture/gpui.md), phase 2: a busy pane's grid renders and a quiet one's is
//! reused from its cache.
mod grid_support;

use std::time::Duration;

use gpui::TestAppContext;
use grid_support::*;

const QUIET: &str = "stty -echo; echo quiet-ready; sleep 300";
/// One process printing, so the pane's title (its program) stays put.
const BUSY: &str = "stty -echo; echo busy-ready; exec perl -e '$|=1; while (1) { print time, \"\\n\"; select(undef, undef, undef, 0.02) }'";

#[gpui::test]
fn a_busy_pane_s_grid_renders_and_a_quiet_one_s_does_not(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let quiet = tab_running(&s, &ws, cx, QUIET, "quiet-ready");
    let busy = split_running(&s, &ws, cx, &quiet, BUSY, "busy-ready");
    run_for(cx, Duration::from_millis(300)); // the split's fit and first screens
    let (q, b) = (grid_renders(&ws, cx, &quiet), grid_renders(&ws, cx, &busy));
    run_for(cx, Duration::from_secs(1));
    let (q, b) = (grid_renders(&ws, cx, &quiet) - q, grid_renders(&ws, cx, &busy) - b);
    s.app.stop();
    assert!(b >= 3, "the busy pane's grid rendered {b} times in 1 s");
    assert_eq!(q, 0, "the quiet pane's grid rendered {q} times beside it");
}
