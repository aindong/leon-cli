//! A focused popover is used by keyboard: Down and Up move the highlight (wrapping), Enter chooses
//! it, Escape dismisses; a click on an item chooses that item.
mod popover_support;

use gpui::{Modifiers, TestAppContext};
use leon_app::popover::PopoverEvent;
use popover_support::*;

#[gpui::test]
fn arrows_move_and_enter_chooses(cx: &mut TestAppContext) {
    let (host, cx) = open_at(cx, rect(100., 100., 40., 20.), true);
    assert_eq!(highlighted(&host, cx), Some(0), "the first item is highlighted when it opens by keyboard");
    cx.simulate_keystrokes("down down");
    assert_eq!(highlighted(&host, cx), Some(2));
    cx.simulate_keystrokes("down");
    assert_eq!(highlighted(&host, cx), Some(0), "wraps to the top");
    cx.simulate_keystrokes("up");
    assert_eq!(highlighted(&host, cx), Some(2), "wraps to the bottom");
    cx.simulate_keystrokes("enter");
    assert_eq!(events(&host, cx), vec![PopoverEvent::Chose("three".into())]);
}

#[gpui::test]
fn escape_dismisses(cx: &mut TestAppContext) {
    let (host, cx) = open_at(cx, rect(100., 100., 40., 20.), true);
    cx.simulate_keystrokes("escape");
    assert_eq!(events(&host, cx), vec![PopoverEvent::Dismissed]);
    assert!(host.read_with(cx, |h, _| h.popover.is_none()), "its owner dropped it");
}

#[gpui::test]
fn a_click_chooses_the_item(cx: &mut TestAppContext) {
    let (host, cx) = open_at(cx, rect(100., 100., 40., 20.), false);
    let b = cx.debug_bounds("popover-item-two").expect("drawn");
    cx.simulate_click(b.center(), Modifiers::none());
    assert_eq!(events(&host, cx), vec![PopoverEvent::Chose("two".into())]);
}

#[gpui::test]
fn unfocused_it_has_no_highlight_until_hovered(cx: &mut TestAppContext) {
    let (host, cx) = open_at(cx, rect(100., 100., 40., 20.), false);
    assert_eq!(highlighted(&host, cx), None, "opened by hover, nothing is highlighted");
    let b = cx.debug_bounds("popover-item-two").expect("drawn");
    cx.simulate_mouse_move(b.center(), None, Modifiers::none());
    assert_eq!(highlighted(&host, cx), Some(1), "the item under the mouse is");
}
