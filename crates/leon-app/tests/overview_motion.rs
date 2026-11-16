//! Overview criterion 13 (.agent/plans/2026-11-17-overview-grid.md): opening and closing move the
//! current card over a slide's time; with Reduce Motion nothing moves, and no frame is drawn
//! between the panes and the grid.

mod m10b_support;

use std::time::Duration;

use gpui::TestAppContext;
use leon_app::floats::SLIDE;
use m10b_support::*;

#[gpui::test]
fn opening_moves_the_current_card_then_settles(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.set_reduce_motion(false));
    act(&ws, cx, "client.tab.new");
    let current = ws.read_with(cx, |ws, _| ws.active_window()).unwrap().id;
    act(&ws, cx, "client.overview.toggle");
    assert!(drawn(cx, "overview-moving"), "the current card moves from the panes to its place");
    // the rest of the grid shows through a veil drawn over the cached thumbnails, never as their
    // opacity: a cached view replays its paint as recorded, so a faint paint would stay faint
    assert!(ws.read_with(cx, |ws, _| ws.overview_veil()) > 0., "a veil over the grid while it opens");
    std::thread::sleep(SLIDE + Duration::from_millis(50));
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert!(!drawn(cx, "overview-moving"), "the motion is over after a slide's time");
    assert!(drawn(cx, &format!("card-{current}")), "the card is in its place");
    assert_eq!(ws.read_with(cx, |ws, _| ws.overview_veil()), 0., "no veil is left over the grid");

    cx.simulate_keystrokes("escape");
    assert!(drawn(cx, "overview-moving"), "closing moves the card back to the panes");
    std::thread::sleep(SLIDE + Duration::from_millis(50));
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert!(!drawn(cx, "overview"), "then the overview is gone");
    s.app.stop();
}

#[gpui::test]
fn reduce_motion_draws_no_frame_in_between(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.set_reduce_motion(true));
    act(&ws, cx, "client.tab.new");
    let current = ws.read_with(cx, |ws, _| ws.active_window()).unwrap().id;
    act(&ws, cx, "client.overview.toggle");
    assert!(!drawn(cx, "overview-moving"), "nothing moves");
    assert!(drawn(cx, &format!("card-{current}")), "the first frame is the grid");
    assert_eq!(ws.read_with(cx, |ws, _| ws.overview_veil()), 0., "no veil either");
    cx.simulate_keystrokes("enter");
    assert!(!drawn(cx, "overview-moving"), "closing moves nothing");
    assert!(!drawn(cx, "overview"), "the panes are back at once");
    assert!(drawn(cx, "panes-area"));
    s.app.stop();
}
