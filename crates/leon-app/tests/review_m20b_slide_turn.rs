//! Review: a drawer turned back mid-slide carries on from where it was (`slide_at` inverts
//! `slide`), and a drawer drag keeps a size set outside 5–95% until it moves.

use leon_app::floats::{slide, slide_at, Grab, Placement, SLIDE};
use leon_core::overlays::DockSide;

#[test]
fn slide_at_inverts_slide() {
    for i in 0..20 {
        let done = i as f32 / 20.;
        let back = slide(slide_at(done), false).unwrap_or(1.);
        assert!((back - done).abs() < 1e-3, "{done} -> {back}");
    }
    assert_eq!(slide_at(1.), SLIDE);
}

#[test]
fn a_tiny_drawer_keeps_its_size_until_dragged() {
    let g = Grab::Dock { side: DockSide::Bottom, size: 0.02, area: (1000., 800.) };
    assert_eq!(g.moved(0., 0.), Placement::Dock(0.02));
}
