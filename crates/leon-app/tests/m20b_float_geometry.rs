//! M20b criterion 2: where drawers and floats go in the pane area, and which one a point hits.

use leon_app::floats::{self, Placed};
use leon_app::panes::{Rect, INSET};
use leon_core::overlays::{Bounds, DockSide, Docked, Floating, Overlays};

fn float(id: &str, b: [f64; 4], hidden: bool) -> Floating {
    Floating { block_id: id.into(), bounds: Bounds { x: b[0], y: b[1], width: b[2], height: b[3] }, hidden }
}

fn dock(id: &str, side: DockSide, size: f64, hidden: bool) -> Docked {
    Docked { block_id: id.into(), side, size, hidden }
}

fn place(o: &Overlays) -> Placed {
    floats::place(o, (1000., 800.), &|_| None)
}

#[test]
fn drawers_reserve_their_side_in_list_order_and_tiles_get_the_rest() {
    let o = Overlays {
        floating: vec![],
        docked: vec![dock("l", DockSide::Left, 0.25, false), dock("b", DockSide::Bottom, 0.3, false)],
    };
    let p = place(&o);
    assert_eq!(p.tiles, Rect { x: 250., y: 0., w: 750., h: 560. });
    let l = &p.docks[0];
    assert_eq!((l.block_id.as_str(), l.side), ("l", DockSide::Left));
    assert_eq!(l.rect, Rect { x: INSET, y: INSET, w: 250. - INSET, h: 800. - 2. * INSET });
    assert_eq!(l.handle, Some(Rect { x: 250., y: 0., w: INSET, h: 800. }));
    // the bottom drawer spans what the left one left, below the tiles
    let b = &p.docks[1];
    assert_eq!(b.rect, Rect { x: 250. + INSET, y: 560., w: 750. - 2. * INSET, h: 240. - INSET });
    assert_eq!(b.handle, Some(Rect { x: 250., y: 560. - INSET, w: 750., h: INSET }));
}

#[test]
fn a_right_drawer_and_a_drawer_too_big_for_what_is_left() {
    let o = Overlays {
        floating: vec![],
        docked: vec![dock("r", DockSide::Right, 0.9, false), dock("l", DockSide::Left, 0.5, false)],
    };
    let p = place(&o);
    assert_eq!(p.docks[0].rect.x, 100., "right drawer starts at its size from the right edge");
    assert_eq!(p.docks[0].rect.w, 900. - INSET);
    // half the width is 500, but only 100 is left
    assert_eq!(p.docks[1].rect.w + INSET, 100.);
    assert_eq!(p.tiles.w, 0.);
}

#[test]
fn hidden_overlays_take_no_room_and_are_not_placed() {
    let o = Overlays {
        floating: vec![float("f", [0.1, 0.1, 0.5, 0.5], true)],
        docked: vec![dock("b", DockSide::Bottom, 0.3, true)],
    };
    let p = place(&o);
    assert_eq!(p.tiles, Rect { x: 0., y: 0., w: 1000., h: 800. });
    assert!(p.docks.is_empty() && p.floats.is_empty(), "{p:?}");
    assert_eq!(p.hit(100., 100.), None);
}

#[test]
fn floats_sit_at_their_bounds_of_the_whole_area_back_to_front() {
    let o = Overlays {
        floating: vec![float("low", [0.1, 0.1, 0.5, 0.5], false), float("top", [0.3, 0.3, 0.5, 0.5], false)],
        docked: vec![dock("b", DockSide::Bottom, 0.25, false)],
    };
    let p = place(&o);
    let ids: Vec<&str> = p.floats.iter().map(|f| f.block_id.as_str()).collect();
    assert_eq!(ids, ["low", "top"]);
    assert_eq!(p.floats[0].rect, Rect { x: 100., y: 80., w: 500., h: 400. });
    assert_eq!(p.floats[1].rect, Rect { x: 300., y: 240., w: 500., h: 400. });
}

#[test]
fn hit_finds_the_topmost_float_then_a_drawer_then_nothing() {
    let o = Overlays {
        floating: vec![float("low", [0.1, 0.1, 0.5, 0.5], false), float("top", [0.3, 0.3, 0.5, 0.5], false)],
        docked: vec![dock("b", DockSide::Bottom, 0.25, false)],
    };
    let p = place(&o);
    assert_eq!(p.hit(150., 100.), Some("low"), "only the lower float is there");
    assert_eq!(p.hit(400., 300.), Some("top"), "both are there: the one on top");
    assert_eq!(p.hit(700., 620.), Some("top"), "a float covers the drawer");
    assert_eq!(p.hit(950., 700.), Some("b"), "the drawer alone");
    assert_eq!(p.hit(950., 50.), None, "a tile: not an overlay");
}

#[test]
fn input_goes_to_a_focused_overlay_over_a_zoomed_tile() {
    let tree = leon_core::layout::Node::block("t");
    let mut w = leon_core::Window {
        id: "w".into(),
        label: None,
        layout: Some(tree),
        focus: Some("f".into()),
        zoomed: Some("t".into()),
    };
    assert_eq!(floats::target(&w), Some("f"), "the focused float, not the zoomed tile");
    w.focus = Some("t".into());
    assert_eq!(floats::target(&w), Some("t"));
    w.focus = None;
    assert_eq!(floats::target(&w), Some("t"), "the zoomed tile when nothing is focused");
}
