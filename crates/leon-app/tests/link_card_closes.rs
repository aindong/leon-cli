//! The hover card closes when the mouse leaves both the link and the card, on any key, and on
//! scroll; moving onto the card keeps it open.
mod link_card_support;

use gpui::{point, Modifiers, MouseButton, ScrollDelta, ScrollWheelEvent, TestAppContext, TouchPhase};
use link_card_support::*;

#[gpui::test]
fn leaving_the_link_closes_it_but_the_card_keeps_it(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let l = links(&s, &ws, cx);
    rest(&ws, cx, l.web, Modifiers::none(), 500);
    let b = at(cx, "popover").expect("the card is drawn");
    cx.simulate_mouse_move(b.center(), None::<MouseButton>, Modifiers::none());
    cx.run_until_parked();
    assert!(card(&ws, cx).is_some(), "on the card it stays");
    cx.simulate_mouse_move(l.web, None::<MouseButton>, Modifiers::none());
    cx.run_until_parked();
    assert!(card(&ws, cx).is_some(), "back on the link it stays");
    cx.simulate_mouse_move(l.off, None::<MouseButton>, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(card(&ws, cx), None, "off both: closed");
    s.app.stop();
}

#[gpui::test]
fn a_key_closes_it(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let l = links(&s, &ws, cx);
    rest(&ws, cx, l.web, Modifiers::none(), 500);
    assert!(card(&ws, cx).is_some());
    cx.simulate_keystrokes("x");
    assert_eq!(card(&ws, cx), None, "a key closes it");
    s.app.stop();
}

#[gpui::test]
fn scrolling_closes_it(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let l = links(&s, &ws, cx);
    rest(&ws, cx, l.web, Modifiers::none(), 500);
    assert!(card(&ws, cx).is_some());
    cx.simulate_event(ScrollWheelEvent {
        position: l.web,
        delta: ScrollDelta::Lines(point(0., 1.)),
        modifiers: Modifiers::none(),
        touch_phase: TouchPhase::Moved,
    });
    cx.run_until_parked();
    assert_eq!(card(&ws, cx), None, "a scroll closes it");
    s.app.stop();
}
