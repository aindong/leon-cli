//! The drawn record must say what the last frame shows: once the overview closes, its thumbnails'
//! panes are not on screen, even while their views are still alive (ADR R10).
mod m10b_support;

use std::time::{Duration, Instant};

use gpui::{TestAppContext, VisualTestContext};
use m10b_support::*;

fn ticks(cx: &mut VisualTestContext, mut pred: impl FnMut(&mut VisualTestContext) -> bool) -> bool {
    let end = Instant::now() + Duration::from_secs(20);
    loop {
        if pred(cx) {
            return true;
        }
        if Instant::now() >= end {
            return false;
        }
        cx.executor().advance_clock(Duration::from_millis(50));
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[gpui::test]
fn a_closed_overview_leaves_no_thumbnail_drawn(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.set_reduce_motion(true)); // closing drops the overview at once
    act(&ws, cx, "pane.split.right");
    let (w, _) = window(&s.app, &ws, cx);
    let ids = leaves(&w);
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ids.iter().all(|id| ws.screen_text(id).is_some()))));
    act(&ws, cx, "client.tab.new");

    act(&ws, cx, "client.overview.toggle");
    let pane = format!("card-pane-{}-{}", w.id, ids[0]);
    let listed = |cx: &mut VisualTestContext| {
        cx.run_until_parked();
        leon_app::view::drawn().contains(&pane)
    };
    assert!(ticks(cx, listed), "{pane}: the thumbnail drew");

    // close it; the update ends by drawing the window (no poll runs in between)
    cx.update(|window, cx| ws.update(cx, |ws, cx| ws.run_action("client.overview.toggle", None, "test", window, cx)));
    let drawn = leon_app::view::drawn();
    s.app.stop();
    assert!(drawn.contains("panes-area"), "the frame after closing drew the panes: {drawn:?}");
    assert!(!drawn.contains(&format!("card-{}", w.id)), "the card itself is gone: {drawn:?}");
    let stale: Vec<_> = drawn.iter().filter(|n| n.starts_with("card-pane-")).collect();
    assert!(stale.is_empty(), "STALE thumbnail names after the overview closed: {stale:?}");
}
