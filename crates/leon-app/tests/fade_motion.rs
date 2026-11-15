//! The fade follows the focus with a transition: the pane losing the focus fades out and the one
//! gaining it fades in, without a jump or a flicker; Reduce Motion shows no frame in between; and
//! a focus that moves on before a fade ended leaves exactly one pane at full strength.
//!
//! The tests set the fade's clock, so no value here depends on how fast the machine is, and they
//! only move the app's own poll timer on: a redraw comes from the app or not at all.

mod m10b_support;

use std::time::{Duration, Instant};

use gpui::{Entity, TestAppContext, VisualTestContext};
use leon_app::floats::SLIDE;
use leon_app::view::Workspace;
use m10b_support::*;

/// The default strength.
const S: f32 = leon_lua::settings::FADE as f32;
/// The app's poll timer.
const POLL: Duration = Duration::from_millis(8);

/// One look at the app: each pane's fade, and whether a fade is moving.
type Look = (Vec<f32>, bool);

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

/// What the app shows once its poll has run with the fade's clock at `at`.
fn look(ws: &Entity<Workspace>, cx: &mut VisualTestContext, ids: &[String], at: Instant) -> Look {
    ws.update(cx, |ws, _| ws.set_fade_clock(at));
    cx.executor().advance_clock(POLL);
    cx.run_until_parked();
    ws.read_with(cx, |ws, _| (ids.iter().map(|id| ws.pane_fade(id)).collect(), ws.fading()))
}

/// `n` panes side by side with the focus on the last and the clock at `t0`, at rest; returns them
/// left to right.
fn panes(
    s: &Setup,
    ws: &Entity<Workspace>,
    cx: &mut VisualTestContext,
    (n, reduce_motion, t0): (usize, bool, Instant),
) -> Vec<String> {
    ws.update(cx, |ws, _| {
        ws.set_reduce_motion(reduce_motion);
        ws.set_fade_clock(t0);
    });
    let mut ids = vec![ws.read_with(cx, |ws, _| ws.target_block()).expect("a pane")];
    while ids.len() < n {
        act(ws, cx, "pane.split.right");
        let new = |cx: &mut VisualTestContext| ws.read_with(cx, |ws, _| ws.target_block()).filter(|t| !ids.contains(t));
        assert!(shows(cx, |cx| new(cx).is_some()), "no split");
        let id = new(cx).unwrap();
        ids.push(id);
    }
    assert_eq!(leaves(&window(&s.app, ws, cx).0), ids, "the panes left to right");
    let mut rest = vec![S; n];
    rest[n - 1] = 0.;
    // the clock stood still, so the fades the splits started are all from t0: draw the last
    // split there, then let them end
    look(ws, cx, &ids, t0);
    let end = t0 + SLIDE;
    assert!(shows(cx, |cx| look(ws, cx, &ids, end) == (rest.clone(), false)), "at rest, the focus on the last pane");
    ids
}

/// With the clock at `at`, move the focus one pane to the left; returns the app's first look that
/// knows. The poll's next tick draws a model that changed, so one look after the model has the
/// new focus is that frame.
fn focus_left(ws: &Entity<Workspace>, cx: &mut VisualTestContext, ids: &[String], at: Instant) -> Look {
    ws.update(cx, |ws, _| ws.set_fade_clock(at));
    let before = ws.read_with(cx, |ws, _| ws.target_block());
    act(ws, cx, "pane.focus.left");
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.target_block()) != before), "the focus didn't move");
    look(ws, cx, ids, at);
    look(ws, cx, ids, at)
}

#[gpui::test]
fn the_fade_moves_between_the_panes_on_the_apps_own_frames(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let t0 = Instant::now();
    let ids = panes(&s, &ws, cx, (2, false, t0));
    let t1 = t0 + ms(1000);
    let start = focus_left(&ws, cx, &ids, t1);
    let mut seen = vec![start.clone()];
    for at in (20..120).step_by(20) {
        seen.push(look(&ws, cx, &ids, t1 + ms(at)));
    }
    let end = look(&ws, cx, &ids, t1 + SLIDE);
    let later = look(&ws, cx, &ids, t1 + ms(5000));
    s.app.stop();
    assert_eq!(start, (vec![S, 0.], true), "the frame that sees the change draws where each pane was");
    for pair in seen.windows(2) {
        let ((was, _), (now, moving)) = (&pair[0], &pair[1]);
        assert!(now[0] < was[0] && now[0] > 0., "the pane gaining the focus fades in: {seen:?}");
        assert!(now[1] > was[1] && now[1] < S, "the pane losing it fades out: {seen:?}");
        assert!(moving, "the app keeps drawing while it fades: {seen:?}");
    }
    assert!(seen[3].0[1] > S / 2., "eased out: past half the way at half the time: {seen:?}");
    assert_eq!(end, (vec![0., S], false), "done 120 ms on, and at rest");
    assert_eq!(later, end, "and nothing moves after");
}

#[gpui::test]
fn reduce_motion_shows_no_frame_in_between(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let t0 = Instant::now();
    let ids = panes(&s, &ws, cx, (2, true, t0));
    let t1 = t0 + ms(1000);
    // every look from the key press on, the clock where a fade would be starting
    ws.update(cx, |ws, _| ws.set_fade_clock(t1));
    act(&ws, cx, "pane.focus.left");
    let mut seen = Vec::new();
    let arrived = shows(cx, |cx| {
        seen.push(look(&ws, cx, &ids, t1));
        seen.last().unwrap().0 == [0., S]
    });
    seen.push(look(&ws, cx, &ids, t1 + ms(60)));
    s.app.stop();
    assert!(arrived, "the fade follows the focus: {seen:?}");
    for (fades, moving) in &seen {
        assert!(!moving, "nothing moves with Reduce Motion: {seen:?}");
        assert!(fades == &[S, 0.] || fades == &[0., S], "only the two end states are ever drawn: {seen:?}");
    }
}

#[gpui::test]
fn a_focus_that_moves_on_mid_fade_leaves_one_pane_at_full_strength(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let t0 = Instant::now();
    let ids = panes(&s, &ws, cx, (3, false, t0));
    let t1 = t0 + ms(1000);
    // from the third pane to the second...
    focus_left(&ws, cx, &ids, t1);
    let mid = look(&ws, cx, &ids, t1 + ms(40));
    // ...and on to the first before that fade ended
    let turned = focus_left(&ws, cx, &ids, t1 + ms(40));
    let mut seen = vec![turned.clone()];
    for at in (60..160).step_by(20) {
        seen.push(look(&ws, cx, &ids, t1 + ms(at)));
    }
    let end = look(&ws, cx, &ids, t1 + ms(40) + SLIDE);
    s.app.stop();
    let (m, moving) = &mid;
    assert!(*moving && m[0] == S && m[1] > 0. && m[1] < S && m[2] > 0. && m[2] < S, "mid-fade: {mid:?}");
    assert_eq!(turned, mid, "nothing jumps when the focus moves on");
    for pair in seen.windows(2) {
        let ((was, _), (now, _)) = (&pair[0], &pair[1]);
        assert!(now[0] <= was[0] && now[1] >= was[1] && now[2] >= was[2], "no flicker: {seen:?}");
        assert!(now.iter().all(|a| (0. ..=S).contains(a)), "{seen:?}");
    }
    assert_eq!(end, (vec![0., S, S], false), "the focus ended on the first pane and nothing is left moving");
    assert_eq!(end.0.iter().filter(|a| **a == 0.).count(), 1, "exactly one pane at full strength");
}
