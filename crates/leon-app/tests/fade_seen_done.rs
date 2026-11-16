//! A finish the user watched happen is seen: once they leave that pane it fades like any other,
//! though the server keeps its `done` until they come back. A new state that comes while they
//! are away needs them again.

mod m10b_support;

use gpui::{Entity, TestAppContext, VisualTestContext};
use leon_app::view::Workspace;
use m10b_support::*;

/// The default strength.
const S: f32 = leon_lua::settings::FADE as f32;
/// The app's poll timer: a wait moves it on and never asks for a redraw itself.
const POLL: std::time::Duration = std::time::Duration::from_millis(8);

/// Pane `id`'s fade once the fades are at rest and it is `want` (else what it was).
fn rests_at(ws: &Entity<Workspace>, cx: &mut VisualTestContext, id: &str, want: f32) -> f32 {
    shows(cx, |cx| {
        cx.executor().advance_clock(POLL);
        cx.run_until_parked();
        ws.read_with(cx, |ws, _| !ws.fading() && ws.pane_fade(id) == want)
    });
    ws.read_with(cx, |ws, _| ws.pane_fade(id))
}

#[gpui::test]
fn a_finish_watched_in_the_focused_pane_fades_once_the_user_leaves(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    // the user is looking at leon: a test window is not the key window until it is activated
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    assert!(cx.update(|window, _| window.is_window_active()), "the window is the key window");
    let first = ws.read_with(cx, |ws, _| ws.target_block()).expect("a pane");
    act(&ws, cx, "pane.split.right");
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.target_block().is_some_and(|t| t != first))), "no split");
    let second = ws.read_with(cx, |ws, _| ws.target_block()).unwrap();
    // the user is on `second` and watches it finish; later it fails, on a line typed to it
    let t = term(&s.app, &second);
    let script = r"printf '\033]7501;state=done\007'; echo sent; read x; printf '\033]7501;state=error\007'; echo again; sleep 300";
    run_until(&t, script, "sent");
    assert!(shows(cx, |cx| drawn(cx, &format!("header-status-{second}"))), "no done indicator");
    let watched = rests_at(&ws, cx, &second, 0.);
    // then moves on to the other pane
    act(&ws, cx, "pane.focus.left");
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.target_block()).as_ref() == Some(&first)), "focus didn't move");
    let left = rests_at(&ws, cx, &second, S);
    let now = ws.read_with(cx, |ws, _| ws.pane_fade(&first));
    // while the user is away, the program fails
    t.write(b"\r").unwrap();
    assert!(eventually(|| has_line(&t, "again")), "the error was never reported");
    let failed = rests_at(&ws, cx, &second, 0.);
    s.app.stop();
    assert_eq!(watched, 0., "the focused pane is at full strength while the user watches it");
    assert_eq!(now, 0., "the pane the user moved to is at full strength");
    assert_eq!(left, S, "the finish the user watched is seen: the pane fades once they leave it");
    assert_eq!(failed, 0., "an error that came while they were away needs them: the pane is not faded");
}

#[gpui::test]
fn a_blocked_pane_the_user_looked_at_stays_bright_after_they_leave(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    let first = ws.read_with(cx, |ws, _| ws.target_block()).expect("a pane");
    act(&ws, cx, "pane.split.right");
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.target_block().is_some_and(|t| t != first))), "no split");
    let second = ws.read_with(cx, |ws, _| ws.target_block()).unwrap();
    // the user looks at an agent that asks them something, then goes elsewhere without answering
    let t = term(&s.app, &second);
    run_until(&t, r"printf '\033]7501;state=blocked:kind=question\007'; echo sent; sleep 300", "sent");
    assert!(shows(cx, |cx| drawn(cx, &format!("header-status-{second}"))), "no blocked indicator");
    rests_at(&ws, cx, &second, 0.);
    act(&ws, cx, "pane.focus.left");
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.target_block()).as_ref() == Some(&first)), "focus didn't move");
    let left = rests_at(&ws, cx, &second, 0.);
    s.app.stop();
    assert_eq!(left, 0., "it is still waiting for the user: a blocked pane never fades");
}
