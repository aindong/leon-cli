//! GPUI phase 5 (ADR R6, R8): what used to ride the poll's deadline clock is a task of its own.
//! The wake handler runs only when another thread wakes it; the Activity list's waited times
//! turn on their own 1 s task, rendering the sidebar and not waking the handler.
mod m20b_support;

use std::time::{Duration, Instant};

use gpui::{Entity, TestAppContext, VisualTestContext};
use leon_app::view::Workspace;
use m20b_support::*;

const BLOCKED: &str = r"printf '\033]7501;state=blocked:kind=permission\007'; echo b-sent; sleep 60";

/// Let `d` of real time pass, the test clock with it; returns how many times the wake handler ran.
fn run_for(ws: &Entity<Workspace>, cx: &mut VisualTestContext, d: Duration) -> u64 {
    cx.run_until_parked();
    let before = ws.read_with(cx, |ws, _| ws.wakes());
    let end = Instant::now() + d;
    while Instant::now() < end {
        let t = Instant::now();
        std::thread::sleep(Duration::from_millis(5));
        tick(ws, cx, t.elapsed());
    }
    ws.read_with(cx, |ws, _| ws.wakes()) - before
}

/// Until 300 ms pass with no wake (what opening brought in landed: a prompt drawn late under load).
fn quiet(ws: &Entity<Workspace>, cx: &mut VisualTestContext) {
    let end = Instant::now() + Duration::from_secs(10);
    while run_for(ws, cx, Duration::from_millis(300)) > 0 && Instant::now() < end {}
}

fn sidebar(ws: &Entity<Workspace>, cx: &mut VisualTestContext) -> u64 {
    ws.read_with(cx, |ws, _| ws.section_renders().get("sidebar").copied().unwrap_or(0))
}

#[gpui::test]
fn an_idle_window_runs_no_wake_handler(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    settle(&focused(&s.app, &ws, cx));
    quiet(&ws, cx);
    let n = run_for(&ws, cx, Duration::from_secs(2));
    s.app.stop();
    assert!(n <= 1, "the wake handler ran {n} times in 2 s with nothing changing");
}

#[gpui::test]
fn the_waited_times_turn_on_their_own_task(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let t = focused(&s.app, &ws, cx);
    run_until(&t, BLOCKED, "b-sent");
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.dock_badge()).is_some()), "the pane is blocked");
    act(&ws, cx, "client.sidebar.activity.toggle");
    cx.simulate_keystrokes("cmd-f b - s e n t");
    let found = |cx: &mut VisualTestContext| ws.read_with(cx, |ws, _| ws.find_status()).is_some_and(|f| f.1 >= 1);
    assert!(shows(cx, found), "the find bar has a match");
    settle(&t);
    quiet(&ws, cx);
    let drew = sidebar(&ws, cx);
    let n = run_for(&ws, cx, Duration::from_secs(2));
    let drew = sidebar(&ws, cx) - drew;
    s.app.stop();
    assert!(n <= 1, "the wake handler ran {n} times in 2 s with find and Activity open and nothing new");
    assert!(drew >= 1, "the Activity list's waited time moved on (the sidebar drew {drew} times)");
}

/// No deadline clock is left in the window: no poll planning its next look, no hash of what is
/// drawn, no keymap clock.
#[test]
fn no_poll_clock_is_left() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for e in std::fs::read_dir(&src).unwrap().flatten() {
        let text = std::fs::read_to_string(e.path()).unwrap_or_default();
        for gone in ["fn signature", "next_poll", "planned:", "KEYMAP_POLL"] {
            assert!(!text.contains(gone), "{} still has {gone}", e.file_name().to_string_lossy());
        }
    }
}
