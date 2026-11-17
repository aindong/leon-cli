//! A drawer sliding out is drawn over tiles that already took its room back
//! (floats::place: "a hidden one (sliding out) takes no room"), so with translucency on,
//! the pane fill, the drawer's fill and the gaps must still put one translucent fill per pixel.

use leon_app::floats::place;
use leon_app::glass::{gaps, see_through_tiles};
use leon_app::panes::{tiles, Rect};
use leon_core::layout::Node;
use leon_core::overlays::{DockSide, Docked, Overlays};

const R: f32 = leon_app::glass::TILE_RADIUS;

fn inside(r: &Rect, (x, y): (f32, f32)) -> bool {
    x > r.x && x < r.x + r.w && y > r.y && y < r.y + r.h
}

/// Inside `t` with its corners rounded by `R` (as `pane_tile`'s `rounded(6)`).
fn in_tile(t: &Rect, p: (f32, f32)) -> bool {
    if !inside(t, p) {
        return false;
    }
    let cx = p.0.clamp(t.x + R, t.x + t.w - R);
    let cy = p.1.clamp(t.y + R, t.y + t.h - R);
    (p.0 - cx).hypot(p.1 - cy) < R
}

#[test]
fn a_drawer_sliding_out_never_stacks_a_second_translucent_fill() {
    let size = (800f32, 600f32);
    let o = Overlays {
        docked: vec![Docked { block_id: "d".into(), side: DockSide::Right, size: 0.3, hidden: true }],
        ..Default::default()
    };
    // the first frames of a hide: most of the drawer still shown
    let placed = place(&o, size, &|_: &str| Some(0.9));
    let tree = Node::Block { block_id: "p".into() };
    let mut l = tiles(&tree, None, (placed.tiles.w, placed.tiles.h));
    for p in &mut l.panes {
        p.rect.x += placed.tiles.x;
        p.rect.y += placed.tiles.y;
    }
    assert!(placed.docks.iter().any(|d| d.leaving), "the drawer is drawn, leaving: {placed:?}");
    // what render_gaps passes and the panes fill at the pane alpha; the leaving drawer is opaque
    let fills = see_through_tiles(&l, &placed, |_| true);
    assert_eq!(fills.len(), 1, "only the tile is translucent: {fills:?}");
    let area = Rect { x: 0., y: 0., w: size.0, h: size.1 };
    let g = gaps(area, &fills, R);
    let mut y = 0.13;
    while y < size.1 {
        let mut x = 0.17;
        while x < size.0 {
            let p = (x, y);
            let panes = fills.iter().filter(|t| in_tile(t, p)).count();
            let rects = g.rects.iter().filter(|r| inside(r, p)).count();
            let corners = g
                .corners
                .iter()
                .filter(|c| inside(&c.clip, p) && (p.0 - c.center.0).hypot(p.1 - c.center.1) > c.radius)
                .count();
            let n = panes + rects + corners;
            assert_eq!(n, 1, "({x}, {y}) is under {n} translucent fills: {fills:?}");
            x += 0.5;
        }
        y += 0.5;
    }
}
