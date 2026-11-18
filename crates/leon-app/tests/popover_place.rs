//! The popover opens under its anchor rect, and flips above (or to the left of) it when it would
//! leave the window, staying wholly inside.
mod popover_support;

use gpui::{px, TestAppContext};
use popover_support::*;

#[gpui::test]
fn opens_under_its_anchor(cx: &mut TestAppContext) {
    let (_, cx) = open_at(cx, rect(100., 100., 40., 20.), false);
    let b = cx.debug_bounds("popover").expect("the popover is drawn");
    assert_eq!((b.origin.x, b.origin.y), (px(100.), px(120.)), "top-left under the rect's bottom-left");
}

#[gpui::test]
fn flips_above_at_the_bottom_edge(cx: &mut TestAppContext) {
    let (_, cx) = open_at(cx, rect(100., 570., 40., 20.), false);
    let b = cx.debug_bounds("popover").expect("the popover is drawn");
    assert_eq!(b.bottom(), px(570.), "its bottom sits on the rect's top: {b:?}");
    assert!(b.top() >= px(0.), "inside the window: {b:?}");
}

#[gpui::test]
fn flips_left_at_the_right_edge(cx: &mut TestAppContext) {
    let (_, cx) = open_at(cx, rect(790., 100., 4., 20.), false);
    let b = cx.debug_bounds("popover").expect("the popover is drawn");
    assert!(b.right() <= px(800.) && b.left() >= px(0.), "inside the window: {b:?}");
    assert_eq!(b.top(), px(120.), "still under the rect: {b:?}");
}

#[gpui::test]
fn a_point_anchor_opens_at_the_point(cx: &mut TestAppContext) {
    let (_, cx) = open_at(cx, rect(200., 150., 0., 0.), false);
    let b = cx.debug_bounds("popover").expect("the popover is drawn");
    assert_eq!((b.origin.x, b.origin.y), (px(200.), px(150.)));
}

#[gpui::test]
fn shows_its_text_and_items(cx: &mut TestAppContext) {
    let (_, cx) = open_at(cx, rect(10., 10., 10., 10.), false);
    for sel in ["popover-item-one", "popover-item-two", "popover-item-three"] {
        assert!(cx.debug_bounds(sel).is_some(), "{sel} drawn");
    }
}
