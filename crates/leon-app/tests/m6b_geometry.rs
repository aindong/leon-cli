//! M6b criterion 1: a layout tree to pane and divider rects.

use leon_app::panes::{layout, Rect};
use leon_core::layout::{Direction, Node, Side};

fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect { x, y, w, h }
}

fn tree(direction: Direction, ratio: f64) -> Node {
    let mut t = Node::block("block:a");
    assert!(t.insert("block:a", "block:b", direction, Side::After, ratio));
    t
}

#[test]
fn a_horizontal_split_with_a_divider() {
    let l = layout(&tree(Direction::Horizontal, 0.5), None, (1001., 600.), 1.);
    let rects: Vec<(&str, Rect)> = l.panes.iter().map(|p| (p.block_id.as_str(), p.rect)).collect();
    assert_eq!(rects, vec![("block:a", rect(0., 0., 500., 600.)), ("block:b", rect(501., 0., 500., 600.))]);
    assert_eq!(l.dividers.len(), 1);
    let d = &l.dividers[0];
    assert_eq!(d.rect, rect(500., 0., 1., 600.));
    assert_eq!((d.direction, d.before.as_str(), d.after.as_str()), (Direction::Horizontal, "block:a", "block:b"));
    assert_eq!(d.split, rect(0., 0., 1001., 600.));
}

#[test]
fn a_vertical_quarter_split() {
    let l = layout(&tree(Direction::Vertical, 0.25), None, (800., 401.), 1.);
    assert_eq!(l.panes[0].rect, rect(0., 0., 800., 100.));
    assert_eq!(l.panes[1].rect, rect(0., 101., 800., 300.));
    assert_eq!(l.dividers[0].rect, rect(0., 100., 800., 1.));
}

#[test]
fn nested_splits_nest() {
    let mut t = tree(Direction::Horizontal, 0.5);
    assert!(t.insert("block:b", "block:c", Direction::Vertical, Side::After, 0.5));
    let l = layout(&t, None, (1001., 601.), 1.);
    let ids: Vec<&str> = l.panes.iter().map(|p| p.block_id.as_str()).collect();
    assert_eq!(ids, ["block:a", "block:b", "block:c"]);
    assert_eq!(l.panes[1].rect, rect(501., 0., 500., 300.));
    assert_eq!(l.panes[2].rect, rect(501., 301., 500., 300.));
    assert_eq!(l.dividers.len(), 2);
    let inner = l.dividers.iter().find(|d| d.direction == Direction::Vertical).unwrap();
    assert_eq!((inner.before.as_str(), inner.after.as_str()), ("block:b", "block:c"));
    assert_eq!(inner.split, rect(501., 0., 500., 601.));
}

#[test]
fn zoom_and_single_blocks_fill_the_area() {
    let l = layout(&tree(Direction::Horizontal, 0.5), Some("block:b"), (900., 500.), 1.);
    assert_eq!(l.panes.len(), 1);
    assert_eq!((l.panes[0].block_id.as_str(), l.panes[0].rect), ("block:b", rect(0., 0., 900., 500.)));
    assert!(l.dividers.is_empty());
    let one = layout(&Node::block("block:a"), None, (900., 500.), 1.);
    assert_eq!(one.panes[0].rect, rect(0., 0., 900., 500.));
    assert!(one.dividers.is_empty());
}
