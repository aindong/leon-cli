//! Review: with two items and none highlighted (a click on the text gave it the keyboard), Up
//! goes to the last item and Down to the first.
mod popover_support;

use gpui::{point, px, Modifiers, TestAppContext};
use leon_app::popover::Item;
use popover_support::*;

fn two() -> Vec<Item> {
    vec![Item::new("a", "A", None), Item::new("b", "B", None)]
}

#[gpui::test]
fn up_from_nothing_goes_to_the_last(cx: &mut TestAppContext) {
    let (host, cx) = open_with(cx, rect(100., 100., 40., 20.), false, two());
    let text = cx.debug_bounds("popover").expect("drawn").origin + point(px(20.), px(8.));
    cx.simulate_click(text, Modifiers::none());
    cx.simulate_keystrokes("up");
    assert_eq!(highlighted(&host, cx), Some(1));
}

#[gpui::test]
fn down_from_nothing_goes_to_the_first(cx: &mut TestAppContext) {
    let (host, cx) = open_with(cx, rect(100., 100., 40., 20.), false, two());
    let text = cx.debug_bounds("popover").expect("drawn").origin + point(px(20.), px(8.));
    cx.simulate_click(text, Modifiers::none());
    cx.simulate_keystrokes("down");
    assert_eq!(highlighted(&host, cx), Some(0));
}
