//! The panes area's gaps, painted translucent: everything in the area that no rounded tile
//! covers is covered exactly once (a second translucent fill on a pixel would add its alpha,
//! GPUI's blend), and nothing inside a tile is.

use leon_app::glass::{gaps, Gaps};
use leon_app::panes::Rect;

const R: f32 = leon_app::glass::TILE_RADIUS;

fn inside(r: &Rect, (x, y): (f32, f32)) -> bool {
    x > r.x && x < r.x + r.w && y > r.y && y < r.y + r.h
}

/// Inside `t` with its corners rounded by `R`.
fn in_tile(t: &Rect, p: (f32, f32)) -> bool {
    if !inside(t, p) {
        return false;
    }
    let cx = p.0.clamp(t.x + R, t.x + t.w - R);
    let cy = p.1.clamp(t.y + R, t.y + t.h - R);
    (p.0 - cx).hypot(p.1 - cy) < R
}

/// How many of `g`'s pieces cover `p`.
fn covers(g: &Gaps, p: (f32, f32)) -> usize {
    let rects = g.rects.iter().filter(|r| inside(r, p)).count();
    let corners = g
        .corners
        .iter()
        .filter(|c| inside(&c.clip, p) && (p.0 - c.center.0).hypot(p.1 - c.center.1) > c.radius)
        .count();
    rects + corners
}

fn check(area: Rect, tiles: &[Rect]) {
    let g = gaps(area, tiles, R);
    // samples off every whole and half pixel, so none sits on an edge
    let mut y = area.y + 0.13;
    while y < area.y + area.h {
        let mut x = area.x + 0.17;
        while x < area.x + area.w {
            let want = usize::from(!tiles.iter().any(|t| in_tile(t, (x, y))));
            assert_eq!(covers(&g, (x, y)), want, "({x}, {y}) in {area:?} with {tiles:?}");
            x += 0.5;
        }
        y += 0.5;
    }
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect { x, y, w, h }
}

#[test]
fn one_tile() {
    check(rect(0., 0., 120., 80.), &[rect(6., 6., 108., 68.)]);
}

#[test]
fn a_split_and_a_drawer() {
    // two tiles side by side, one of them split down, and a drawer along the bottom
    let tiles = [rect(6., 6., 60., 50.), rect(72., 6., 60., 22.), rect(72., 34., 60., 22.), rect(6., 62., 126., 30.)];
    check(rect(0., 0., 138., 98.), &tiles);
}

#[test]
fn no_tiles_is_all_gap() {
    let area = rect(10., 20., 50., 30.);
    let g = gaps(area, &[], R);
    assert!(g.corners.is_empty());
    check(area, &[]);
}

#[test]
fn a_tile_past_the_edge_is_cut_to_the_area() {
    // a drawer sliding in starts outside the area
    check(rect(0., 0., 100., 60.), &[rect(70., 6., 60., 48.), rect(6., 6., 58., 48.)]);
}
