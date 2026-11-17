//! Motion PR 1 (ADR R3, R6, R10): a pane's fade moves on GPUI's clock, one step per frame the
//! app asks for, and a grid whose fade doesn't move doesn't render.
mod grid_support;

use std::time::Duration;

use gpui::{Entity, TestAppContext, VisualTestContext};
use grid_support::*;
use leon_app::view::Workspace;

const FRAME: Duration = Duration::from_millis(16);
const S: f32 = leon_lua::settings::FADE as f32;

fn fades(ws: &Entity<Workspace>, cx: &mut VisualTestContext, ids: &[&str]) -> Vec<f32> {
    ws.read_with(cx, |ws, _| ids.iter().map(|id| ws.pane_fade(id)).collect())
}

#[gpui::test]
fn a_focus_move_fades_two_panes_and_leaves_the_third_alone(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.set_reduce_motion(false));
    let a = tab_running(&s, &ws, cx, &quiet("a-ready"), "a-ready");
    let b = split_running(&s, &ws, cx, &a, &quiet("b-ready"), "b-ready");
    let c = split_running(&s, &ws, cx, &b, &quiet("c-ready"), "c-ready");
    // at rest: the focus on c, a and b faded
    while ws.read_with(cx, |ws, _| ws.frame_asked()) {
        frame(&ws, cx, FRAME);
    }
    assert_eq!(fades(&ws, cx, &[&a, &b, &c]), vec![S, S, 0.], "a and b faded, c in focus");
    // the splits' screens settle (a watch set that changed re-sends screens) before counting
    run_for(cx, Duration::from_millis(300));
    let a_renders = grid_renders(&ws, cx, &a);
    act(&ws, cx, "pane.focus.left"); // to b
    let mut steps = vec![fades(&ws, cx, &[&b, &c])];
    let mut n = 0;
    while ws.read_with(cx, |ws, _| ws.frame_asked()) {
        frame(&ws, cx, FRAME);
        steps.push(fades(&ws, cx, &[&b, &c]));
        n += 1;
        assert!(n < 100, "the fade never ended");
    }
    let a_after = grid_renders(&ws, cx, &a) - a_renders;
    let fading = ws.read_with(cx, |ws, _| ws.fading());
    s.app.stop();
    let between = |x: f32| x > 0. && x < S;
    assert!(steps.iter().any(|f| between(f[0]) && between(f[1])), "both moved through the middle: {steps:?}");
    assert!(steps.windows(2).all(|w| w[1][0] <= w[0][0] && w[1][1] >= w[0][1]), "b brightens, c fades: {steps:?}");
    assert_eq!(steps.last().unwrap(), &vec![0., S], "they end at their targets");
    assert!(!fading, "the fade is over");
    assert_eq!(a_after, 0, "a's grid, whose fade didn't move, didn't render");
}
