//! A popover is dismissed by a press outside it or by its focus moving elsewhere; a press inside
//! it is not one.
mod popover_support;

use gpui::{point, px, Modifiers, MouseButton, TestAppContext};
use leon_app::popover::PopoverEvent;
use popover_support::*;

#[gpui::test]
fn a_press_outside_dismisses(cx: &mut TestAppContext) {
    let (host, cx) = open_at(cx, rect(100., 100., 40., 20.), false);
    // on its text, not an item
    let inside = cx.debug_bounds("popover").expect("drawn").origin + point(px(20.), px(8.));
    cx.simulate_mouse_down(inside, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(inside, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(events(&host, cx), vec![], "a press inside keeps it");
    cx.simulate_click(point(px(700.), px(500.)), Modifiers::none());
    assert_eq!(events(&host, cx).last(), Some(&PopoverEvent::Dismissed));
}

#[gpui::test]
fn a_right_press_outside_dismisses_too(cx: &mut TestAppContext) {
    let (host, cx) = open_at(cx, rect(100., 100., 40., 20.), false);
    cx.simulate_mouse_down(point(px(700.), px(500.)), MouseButton::Right, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(events(&host, cx), vec![PopoverEvent::Dismissed]);
}

#[gpui::test]
fn focus_moving_elsewhere_dismisses(cx: &mut TestAppContext) {
    let (host, cx) = open_at(cx, rect(100., 100., 40., 20.), true);
    host.update_in(cx, |host, window, _| window.focus(&host.focus));
    cx.run_until_parked();
    assert_eq!(events(&host, cx), vec![PopoverEvent::Dismissed]);
}
