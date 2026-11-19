//! Overview criterion 10 (.agent/plans/2026-11-17-overview-grid.md): an idle overview leaves the
//! window idle, and a busy pane's card refreshes at most 4 times a second, alone.

mod m10b_support;

use std::time::{Duration, Instant};

use gpui::{TestAppContext, VisualTestContext};
use m10b_support::*;
use serde_json::json;

fn all_filled(ws: &gpui::Entity<leon_app::view::Workspace>, cx: &mut VisualTestContext) -> bool {
    ws.read_with(cx, |ws, _| ws.overview_cards().iter().all(|c| ws.thumb_filled(&c.window_id)))
}

/// `shows`, with the test clock moving too: a refresh `THUMB_EVERY` held back is a timer on it.
fn ticks(cx: &mut VisualTestContext, mut pred: impl FnMut(&mut VisualTestContext) -> bool) -> bool {
    let end = Instant::now() + Duration::from_secs(20);
    loop {
        // the answer it saw, not a second look: a frame between two looks can change it
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

/// Let what opening brought in land (the whole screens a new watch sends, a prompt still being
/// drawn, the refreshes the throttle held back): until nothing has refreshed for 10 ticks.
fn settle(ws: &gpui::Entity<leon_app::view::Workspace>, cx: &mut VisualTestContext) {
    let (mut last, mut still) = (None, 0);
    let end = Instant::now() + Duration::from_secs(20);
    while still < 10 && Instant::now() < end {
        cx.executor().advance_clock(Duration::from_millis(50));
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(30));
        let now = Some(ws.read_with(cx, |ws, _| (ws.thumb_refreshes(), ws.renders())));
        (still, last) = if now == last { (still + 1, last) } else { (0, now) };
    }
}

#[gpui::test]
fn an_idle_overview_changes_nothing(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let ci = s.app.call("session.create", json!({ "label": "ci" })).unwrap();
    act(&ws, cx, "client.tab.new");
    // every shell at its prompt first: a prompt drawn late is a change
    let mut blocks = vec![ci["block_id"].as_str().unwrap().to_string()];
    blocks.extend(ws.read_with(cx, |ws, _| ws.target_block()));
    for (i, b) in blocks.iter().enumerate() {
        run_until(&term(&s.app, b), &format!("echo ready-{i}"), &format!("ready-{i}"));
    }
    act(&ws, cx, "client.overview.toggle");
    assert!(ticks(cx, |cx| all_filled(&ws, cx)), "every card gets its screen");
    settle(&ws, cx);
    let first = ws.read_with(cx, |ws, _| (ws.renders(), ws.thumb_refreshes()));
    let mut later = Vec::new();
    for _ in 0..10 {
        cx.executor().advance_clock(Duration::from_millis(100));
        std::thread::sleep(Duration::from_millis(30));
        cx.run_until_parked();
        later.push(ws.read_with(cx, |ws, _| (ws.renders(), ws.thumb_refreshes())));
    }
    s.app.stop();
    assert!(later.iter().all(|l| *l == first), "the overview moved while nothing changed: {first:?} then {later:?}");
}

#[gpui::test]
fn a_busy_card_refreshes_a_few_times_a_second_and_alone(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let quiet = ws.read_with(cx, |ws, _| ws.active_window()).unwrap().id;
    let quiet_block = ws.read_with(cx, |ws, _| ws.target_block()).unwrap();
    run_until(&term(&s.app, &quiet_block), "echo quiet-ready", "quiet-ready");
    act(&ws, cx, "client.tab.new");
    let busy = ws.read_with(cx, |ws, _| ws.active_window()).unwrap();
    let block = ws.read_with(cx, |ws, _| ws.target_block()).unwrap();
    run_until(&term(&s.app, &block), "echo ready", "ready");
    act(&ws, cx, "client.overview.toggle");
    assert!(ticks(cx, |cx| all_filled(&ws, cx)));
    settle(&ws, cx);
    term(&s.app, &block).write(b"while :; do date; done\r").unwrap();
    let start = ws.read_with(cx, |ws, _| ws.thumb_refreshes());
    let renders = ws.read_with(cx, |ws, _| ws.thumb_renders());
    let t0 = Instant::now();
    while t0.elapsed() < Duration::from_secs(2) {
        cx.executor().advance_clock(Duration::from_millis(16));
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(8));
    }
    let secs = t0.elapsed().as_secs_f64();
    let end = ws.read_with(cx, |ws, _| ws.thumb_refreshes());
    term(&s.app, &block).write(b"\x03").unwrap();
    // the window draws what the refreshes asked for (a draw may wait on the test clock)
    let before = renders[&busy.id];
    ticks(cx, |cx| ws.read_with(cx, |ws, _| ws.thumb_renders().get(&busy.id).copied().unwrap_or(0) > before));
    let rendered = ws.read_with(cx, |ws, _| ws.thumb_renders());
    s.app.stop();
    let grew = |w: &str| end.get(w).copied().unwrap_or(0) - start.get(w).copied().unwrap_or(0);
    assert!(grew(&busy.id) >= 2, "the busy card refreshed: {start:?} → {end:?}");
    assert!(grew(&busy.id) as f64 <= 4. * secs + 1., "at most 4 a second: {} in {secs:.2} s", grew(&busy.id));
    assert_eq!(grew(&quiet), 0, "the quiet card was not refreshed");
    // the window drew each refresh, and the quiet card's cached view replayed its last paint
    assert!(rendered[&busy.id] > renders[&busy.id], "the busy card drew again: {renders:?} → {rendered:?}");
    assert_eq!(rendered[&quiet], renders[&quiet], "the quiet card's thumbnail never drew again");
}
