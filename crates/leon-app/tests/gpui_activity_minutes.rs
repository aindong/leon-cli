//! The Activity list's waited time keeps moving on an idle window past the seconds: "1m" turns
//! "2m" with nothing else drawing (the label stays the same for 60 ticks in between).
mod m20b_support;

use std::time::Duration;

use gpui::{Entity, TestAppContext, VisualTestContext};
use leon_app::view::Workspace;
use m20b_support::*;

const BLOCKED: &str = r"printf '\033]7501;state=blocked:kind=permission\007'; echo b-sent; sleep 600";

fn sidebar(ws: &Entity<Workspace>, cx: &mut VisualTestContext) -> u64 {
    ws.read_with(cx, |ws, _| ws.section_renders().get("sidebar").copied().unwrap_or(0))
}

/// Move GPUI's clock on `secs` seconds, a second at a time, drawing any frame the app asks for.
fn pass(ws: &Entity<Workspace>, cx: &mut VisualTestContext, secs: u64) {
    for _ in 0..secs {
        tick(ws, cx, Duration::from_secs(1));
    }
}

#[gpui::test]
fn the_waited_time_turns_from_one_minute_to_two(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let t = focused(&s.app, &ws, cx);
    run_until(&t, BLOCKED, "b-sent");
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.dock_badge()).is_some()), "the pane is blocked");
    act(&ws, cx, "client.sidebar.activity.toggle");
    let listed = ws.read_with(cx, |ws, _| ws.activity_visible() && !ws.activity_rows().is_empty());
    assert!(listed, "the Activity list is open on the blocked pane");
    settle(&t);
    cx.run_until_parked();
    pass(&ws, cx, 65); // past "1m"
    let at_one = sidebar(&ws, cx);
    pass(&ws, cx, 60); // past 120 s: "2m"
    let at_two = sidebar(&ws, cx);
    s.app.stop();
    assert!(
        at_two > at_one,
        "the Activity list's waited time never turned from 1m to 2m (sidebar renders {at_one} -> {at_two})"
    );
}
