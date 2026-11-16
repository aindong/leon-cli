//! Themes criterion 6 (.agent/plans/2026-11-17-themes.md): a theme change cross-fades over a
//! slide's time, the panes' text dipping toward the background mid-way, and ends drawing the new
//! theme with nothing left moving; with Reduce Motion the first frame is the new theme.
//!
//! The tests set the fade's clock, so no value depends on how fast the machine is; a frame is drawn
//! by notifying the workspace (the test platform runs no animation frames).

mod m10b_support;

use std::time::{Duration, Instant};

use gpui::{Entity, TestAppContext, VisualTestContext};
use leon_app::floats::SLIDE;
use leon_app::theme::{Theme, DARK, LIGHT};
use leon_app::view::Workspace;
use m10b_support::*;

/// A frame drawn with the fade's clock at `at`: (the colours the views draw with, still moving, the
/// pane's fade). The pane's own theme (`pane_theme`) is what its tile paints, from the same field.
fn frame(ws: &Entity<Workspace>, cx: &mut VisualTestContext, pane: &str, at: Instant) -> (Theme, bool, f32) {
    ws.update(cx, |ws, cx| {
        ws.set_fade_clock(at);
        cx.notify();
    });
    cx.run_until_parked();
    ws.read_with(cx, |ws, _| {
        let fade = ws.pane_fade(pane);
        assert_eq!(ws.pane_theme(pane), leon_app::fade::theme(&ws.drawn_theme(), fade), "the tile draws it");
        (ws.drawn_theme(), ws.theme_shifting(), fade)
    })
}

#[gpui::test]
fn a_change_cross_fades_then_rests(cx: &mut TestAppContext) {
    let s = setup_with_config("-- mine\n");
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.sync_keymap());
    let pane = ws.read_with(cx, |ws, _| ws.target_block()).expect("a pane");
    let t0 = Instant::now();
    ws.update(cx, |ws, _| {
        ws.set_reduce_motion(false);
        ws.set_fade_clock(t0);
    });
    let rest = frame(&ws, cx, &pane, t0);
    act(&ws, cx, "client.theme.change");
    cx.simulate_keystrokes("down"); // leon-light, at t0
    let start = frame(&ws, cx, &pane, t0);
    let mid = frame(&ws, cx, &pane, t0 + SLIDE / 2);
    let end = frame(&ws, cx, &pane, t0 + SLIDE);
    let later = frame(&ws, cx, &pane, t0 + SLIDE + Duration::from_secs(1));
    s.app.stop();

    assert_eq!(rest, (DARK, false, 0.), "at rest in leon-dark");
    assert_eq!((start.0, start.1), (DARK, true), "the change starts from the colours on screen");
    assert!(mid.1 && mid.0 != DARK && mid.0 != LIGHT, "mid-way the colours are between: {:06x}", mid.0.bg);
    let between = |a: u32, b: u32, c: u32| (a.min(b)..=a.max(b)).contains(&c);
    assert!(between(DARK.bg >> 8 & 0xff, LIGHT.bg >> 8 & 0xff, mid.0.bg >> 8 & 0xff), "{:06x}", mid.0.bg);
    assert!(mid.2 > 0.3, "the pane's text dips toward its background mid-way: {}", mid.2);
    assert_eq!(end, (LIGHT, false, 0.), "after the slide time: leon-light, nothing moving");
    assert_eq!(later, end, "and it stays so");
}

#[gpui::test]
fn reduce_motion_draws_no_frame_in_between(cx: &mut TestAppContext) {
    let s = setup_with_config("-- mine\n");
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.sync_keymap());
    let pane = ws.read_with(cx, |ws, _| ws.target_block()).expect("a pane");
    let t0 = Instant::now();
    ws.update(cx, |ws, _| {
        ws.set_reduce_motion(true);
        ws.set_fade_clock(t0);
    });
    act(&ws, cx, "client.theme.change");
    cx.simulate_keystrokes("down");
    let first = ws.read_with(cx, |ws, _| (ws.drawn_theme(), ws.theme_shifting(), ws.pane_fade(&pane)));
    cx.simulate_keystrokes("escape");
    let back = ws.read_with(cx, |ws, _| (ws.drawn_theme(), ws.theme_shifting()));
    s.app.stop();
    assert_eq!(first, (LIGHT, false, 0.), "the first frame is the new theme");
    assert_eq!(back, (DARK, false), "and esc's first frame the old one");
}

#[gpui::test]
fn the_first_settings_are_drawn_at_once(cx: &mut TestAppContext) {
    let s = setup_with_config("leon.set(\"theme\", \"leon-light\")\n");
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.set_reduce_motion(false));
    ws.update(cx, |ws, _| ws.sync_keymap());
    // the look as soon as the settings land: a fade from leon-dark would still be moving
    let mut first = None;
    let landed = shows(cx, |cx| {
        let now = ws.read_with(cx, |ws, _| (ws.theme(), ws.drawn_theme(), ws.theme_shifting()));
        first = Some(now);
        now.0 == LIGHT
    });
    s.app.stop();
    assert!(landed, "the settings arrive");
    assert_eq!(first, Some((LIGHT, LIGHT, false)), "drawn at once, not faded in from leon-dark");
}
