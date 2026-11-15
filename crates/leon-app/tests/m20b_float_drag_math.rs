//! M20b criterion 3: dragging a float's header moves it, an edge resizes it, a drawer's divider
//! sets its size; every result is bounds the server accepts.

use leon_app::floats::{Edges, Grab, Placement, MIN_FLOAT};
use leon_core::overlays::{Bounds, DockSide};

const AREA: (f32, f32) = (1000., 800.);

fn b(x: f64, y: f64, w: f64, h: f64) -> Bounds {
    Bounds { x, y, width: w, height: h }
}

fn float(p: Placement) -> Bounds {
    match p {
        Placement::Float(b) => b,
        other => panic!("not a float: {other:?}"),
    }
}

fn close(a: Bounds, want: Bounds) -> bool {
    [a.x - want.x, a.y - want.y, a.width - want.width, a.height - want.height].iter().all(|d| d.abs() < 1e-9)
}

#[test]
fn a_header_drag_moves_by_the_pointer_and_keeps_the_size() {
    let g = Grab::Move { bounds: b(0.1, 0.1, 0.5, 0.5), area: AREA };
    let got = float(g.moved(100., 80.));
    assert!(close(got, b(0.2, 0.2, 0.5, 0.5)), "{got:?}");
    let got = float(g.moved(-50., -40.));
    assert!(close(got, b(0.05, 0.05, 0.5, 0.5)), "{got:?}");
}

#[test]
fn a_move_stops_at_the_area_edge() {
    let g = Grab::Move { bounds: b(0.1, 0.1, 0.5, 0.5), area: AREA };
    let got = float(g.moved(5000., 5000.));
    assert!(close(got, b(0.5, 0.5, 0.5, 0.5)), "{got:?}");
    let got = float(g.moved(-5000., -5000.));
    assert!(close(got, b(0., 0., 0.5, 0.5)), "{got:?}");
}

#[test]
fn an_edge_drag_moves_only_that_edge() {
    let start = b(0.2, 0.2, 0.4, 0.4);
    let right = Grab::Resize { bounds: start, edges: Edges { right: true, ..Edges::default() }, area: AREA };
    assert!(close(float(right.moved(100., 50.)), b(0.2, 0.2, 0.5, 0.4)));
    let bottom = Grab::Resize { bounds: start, edges: Edges { bottom: true, ..Edges::default() }, area: AREA };
    assert!(close(float(bottom.moved(30., 80.)), b(0.2, 0.2, 0.4, 0.5)));
    let left = Grab::Resize { bounds: start, edges: Edges { left: true, ..Edges::default() }, area: AREA };
    assert!(close(float(left.moved(-100., 0.)), b(0.1, 0.2, 0.5, 0.4)), "the right edge stays");
    let corner = Grab::Resize { bounds: start, edges: Edges { top: true, left: true, ..Edges::default() }, area: AREA };
    assert!(close(float(corner.moved(100., 80.)), b(0.3, 0.3, 0.3, 0.3)));
}

#[test]
fn a_resize_keeps_a_minimum_size_and_stays_inside() {
    let start = b(0.2, 0.2, 0.4, 0.4);
    let (mw, mh) = (MIN_FLOAT.0 as f64 / 1000., MIN_FLOAT.1 as f64 / 800.);
    let right = Grab::Resize { bounds: start, edges: Edges { right: true, ..Edges::default() }, area: AREA };
    assert!(close(float(right.moved(-5000., 0.)), b(0.2, 0.2, mw, 0.4)), "no narrower than the minimum");
    assert!(close(float(right.moved(5000., 0.)), b(0.2, 0.2, 0.8, 0.4)), "no wider than the area");
    let left = Grab::Resize { bounds: start, edges: Edges { left: true, ..Edges::default() }, area: AREA };
    assert!(close(float(left.moved(5000., 0.)), b(0.6 - mw, 0.2, mw, 0.4)), "the left edge stops short");
    let top = Grab::Resize { bounds: start, edges: Edges { top: true, ..Edges::default() }, area: AREA };
    assert!(close(float(top.moved(0., -5000.)), b(0.2, 0., 0.4, 0.6)));
    assert!(close(float(top.moved(0., 5000.)), b(0.2, 0.6 - mh, 0.4, mh)));
}

#[test]
fn every_drag_gives_bounds_the_server_accepts() {
    let starts = [b(0.1, 0.1, 0.8, 0.8), b(0.3, 0.7, 0.7, 0.3), b(0., 0., 1., 1.), b(0.123, 0.456, 0.333, 0.222)];
    let all = Edges { left: true, right: true, top: true, bottom: true };
    for s in starts {
        for i in -40..=40 {
            let (dx, dy) = (i as f32 * 37.3, i as f32 * -23.9);
            for g in [
                Grab::Move { bounds: s, area: AREA },
                Grab::Resize { bounds: s, edges: Edges { right: true, bottom: true, ..Edges::default() }, area: AREA },
                Grab::Resize { bounds: s, edges: Edges { left: true, top: true, ..Edges::default() }, area: AREA },
                Grab::Resize { bounds: s, edges: all, area: AREA },
            ] {
                let got = float(g.moved(dx, dy));
                assert!(got.check().is_ok(), "{g:?} by ({dx}, {dy}) gave {got:?}");
            }
        }
    }
}

#[test]
fn a_drawer_divider_drag_sets_its_size_by_side() {
    let size = |p: Placement| match p {
        Placement::Dock(s) => s,
        other => panic!("not a dock: {other:?}"),
    };
    let left = Grab::Dock { side: DockSide::Left, size: 0.3, area: AREA };
    assert!((size(left.moved(100., 0.)) - 0.4).abs() < 1e-9, "dragging right widens a left drawer");
    let right = Grab::Dock { side: DockSide::Right, size: 0.3, area: AREA };
    assert!((size(right.moved(100., 0.)) - 0.2).abs() < 1e-9, "dragging right narrows a right drawer");
    let bottom = Grab::Dock { side: DockSide::Bottom, size: 0.3, area: AREA };
    assert!((size(bottom.moved(0., -80.)) - 0.4).abs() < 1e-9, "dragging up grows a bottom drawer");
    assert_eq!(size(bottom.moved(0., -5000.)), 0.95);
    assert_eq!(size(bottom.moved(0., 5000.)), 0.05);
}
