//! M11 criterion 7: a program that asked for the mouse gets clicks and the wheel; shift selects
//! anyway; an alt-screen program with no mouse mode gets the wheel as arrow keys.
mod m11_support;

use gpui::{point, Modifiers, ScrollDelta, ScrollWheelEvent, TestAppContext, TouchPhase};
use m11_support::*;

/// `cat -v` shows what the program reads: ESC as `^[`.
const TRACKING: &str = "stty -echo -icanon; printf '\\033[?1000h\\033[?1006hready\\n'; cat -v";
const ALT_SCREEN: &str = "stty -echo -icanon; printf '\\033[?1049hready\\n'; cat -v";

fn wheel_up(cx: &mut gpui::VisualTestContext, at: gpui::Point<gpui::Pixels>, lines: f32) {
    let ev = ScrollWheelEvent {
        position: at,
        delta: ScrollDelta::Lines(point(0., lines)),
        modifiers: Modifiers::none(),
        touch_phase: TouchPhase::Moved,
    };
    cx.simulate_event(ev);
    cx.run_until_parked();
}

#[gpui::test]
fn clicks_go_to_a_program_that_asked_and_shift_selects(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let id = tab_running(&s, &ws, cx, TRACKING, "ready");
    let t = term(&s.app, &id);

    let (from, to) = (cell(&ws, cx, &id, 2, 3), cell(&ws, cx, &id, 2, 3));
    drag(cx, from, to, Modifiers::none());
    assert!(eventually(|| t.text(true).contains("^[[<0;4;3M^[[<0;4;3m")), "the click as SGR: {}", t.text(true));
    assert_eq!(ws.read_with(cx, |ws, _| ws.highlighted_text(&id)), None, "and no selection");

    let before = t.text(true);
    let (from, to) = (cell(&ws, cx, &id, 0, 0), cell(&ws, cx, &id, 0, 4));
    drag(cx, from, to, shift());
    cx.simulate_keystrokes("cmd-c");
    assert_eq!(clipboard(cx).as_deref(), Some("ready"), "shift+drag selects anyway");
    std::thread::sleep(std::time::Duration::from_millis(200));
    assert_eq!(t.text(true), before, "and the program got nothing");

    let at = cell(&ws, cx, &id, 1, 1);
    wheel_up(cx, at, 1.);
    assert!(eventually(|| t.text(true).contains("^[[<64;2;2M")), "the wheel as button 64: {}", t.text(true));
    assert_eq!(ws.read_with(cx, |ws, _| ws.view_top(&id)), None, "the view didn't scroll");
    s.app.stop();
}

#[gpui::test]
fn the_wheel_scrolls_an_alt_screen_program(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let id = tab_running(&s, &ws, cx, ALT_SCREEN, "ready");
    let t = term(&s.app, &id);
    let at = cell(&ws, cx, &id, 1, 1);
    wheel_up(cx, at, 3.);
    assert!(eventually(|| t.text(true).contains("^[[A^[[A^[[A")), "three up arrows: {}", t.text(true));
    assert_eq!(ws.read_with(cx, |ws, _| ws.view_top(&id)), None);
    s.app.stop();
}
