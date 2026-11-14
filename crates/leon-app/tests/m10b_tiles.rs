//! M10b criterion 7: panes are inset tiles with a gap between them; the focused one of several is
//! marked by an edge on its left side, not a full outline.

mod m10b_support;

use gpui::TestAppContext;
use leon_app::panes::{tiles, Rect, GAP, INSET};
use leon_core::layout::{Direction, Node, Side};
use m10b_support::*;

#[test]
fn tiles_are_inset_and_apart() {
    assert_eq!((GAP, INSET), (6., 6.));
    let mut t = Node::block("block:a");
    assert!(t.insert("block:a", "block:b", Direction::Horizontal, Side::After, 0.5));
    let l = tiles(&t, None, (1012., 600.));
    let r = |x, y, w, h| Rect { x, y, w, h };
    assert_eq!(l.panes[0].rect, r(6., 6., 497., 588.));
    assert_eq!(l.panes[1].rect, r(509., 6., 497., 588.));
    assert_eq!(l.dividers[0].rect, r(503., 6., 6., 588.), "the gap is the divider you drag");
    let one = tiles(&Node::block("block:a"), None, (800., 500.));
    assert_eq!(one.panes[0].rect, r(6., 6., 788., 488.), "a lone pane is inset too");
}

#[gpui::test]
fn the_focused_pane_of_several_has_a_left_edge(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let a = ws.read_with(cx, |ws, _| ws.target_block()).unwrap();
    assert!(!drawn(cx, &format!("focus-edge-{a}")), "one pane: no focus mark");
    cx.simulate_keystrokes("cmd-d");
    let b = ws.read_with(cx, |ws, _| ws.target_block()).unwrap();
    let edge = at(cx, &format!("focus-edge-{b}")).expect("the focused pane is marked");
    let pane = at(cx, &format!("pane-{b}")).unwrap();
    assert!(!drawn(cx, &format!("focus-edge-{a}")), "only the focused one");
    assert_eq!(edge.left(), pane.left(), "on its left side");
    assert!(f32::from(edge.size.width) <= 3., "a thin edge, not an outline: {edge:?}");
    let pa = at(cx, &format!("pane-{a}")).unwrap();
    assert!(f32::from(pane.left() - pa.right()) >= 6., "a gap between the tiles");
    s.app.stop();
}
