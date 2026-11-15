//! Audit A6 (docs/audit-0.1.0-app.md): with two buttons held down and reported, letting go of the
//! second reports the second's release, and the first is still the one held.
mod m11_support;

use gpui::{Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, TestAppContext};
use m11_support::*;

/// The SGR mouse reports `cat -v` echoed, as `b;col;row` and `M` (press) or `m` (release).
fn reports(text: &str) -> Vec<String> {
    text.split("^[[<")
        .skip(1)
        .map(|r| r.chars().take_while(|c| c.is_ascii_digit() || matches!(c, ';' | 'M' | 'm')).collect())
        .collect()
}

#[gpui::test]
fn letting_go_of_the_second_button_reports_that_button(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let script = "stty -echo -icanon; printf '\\033[?1000h\\033[?1006hready\\n'; exec cat -v";
    let id = tab_running(&s, &ws, cx, script, "ready");
    let at = cell(&ws, cx, &id, 3, 10);
    let none = Modifiers::none();
    let down = |b| MouseDownEvent { button: b, position: at, modifiers: none, click_count: 1, first_mouse: false };
    cx.simulate_event(down(MouseButton::Left));
    cx.simulate_event(down(MouseButton::Right));
    cx.simulate_event(MouseUpEvent { button: MouseButton::Right, position: at, modifiers: none, click_count: 1 });
    cx.simulate_event(MouseUpEvent { button: MouseButton::Left, position: at, modifiers: none, click_count: 1 });
    let seen =
        |cx: &mut gpui::VisualTestContext| reports(&ws.read_with(cx, |ws, _| ws.view_text(&id)).unwrap_or_default());
    let all = shows(cx, |cx| seen(cx).len() >= 4);
    let got: Vec<(char, char)> =
        seen(cx).iter().map(|r| (r.chars().next().unwrap_or('?'), r.chars().last().unwrap_or('?'))).collect();
    assert!(all, "four reports: {got:?}");
    assert_eq!(
        got,
        [('0', 'M'), ('2', 'M'), ('2', 'm'), ('0', 'm')],
        "press left, press right, release right, then left"
    );
    s.app.stop();
}
