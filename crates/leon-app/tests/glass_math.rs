//! The translucent window's alphas, the GPUI-free part: 0 is today's opaque window exactly, and
//! more translucency never makes a fill more opaque.

use leon_app::fade;
use leon_app::glass::Glass;
use leon_app::theme::{DARK, LIGHT};

#[test]
fn zero_is_opaque_everywhere() {
    for t in [DARK, LIGHT] {
        let g = Glass::new(&t, 0.);
        assert_eq!(g.chrome_alpha(), 1., "the chrome is opaque at 0");
        assert_eq!(g.pane_alpha(&t), 1., "a pane is opaque at 0");
        assert_eq!(g.pane_alpha(&fade::theme(&t, 0.35)), 1., "a faded pane too");
        assert!(!g.translucent());
    }
    assert!(!Glass::OPAQUE.translucent());
    assert_eq!(Glass::OPAQUE.chrome_alpha(), 1.);
}

#[test]
fn more_translucency_never_makes_a_fill_more_opaque() {
    for t in [DARK, LIGHT] {
        let mut last = (1f32, 1f32);
        for i in 0..=16 {
            let s = i as f32 * 0.05;
            let g = Glass::new(&t, s);
            let now = (g.chrome_alpha(), g.pane_alpha(&t));
            assert!(now.0 <= last.0 && now.1 <= last.1, "{s}: {now:?} after {last:?}");
            assert!(now.0 >= 1. - s - 1e-6 && now.0 <= 1., "{s}: the chrome asks for 1 - s at most: {now:?}");
            assert!((0. ..=1.).contains(&now.1));
            last = now;
        }
        assert!(Glass::new(&t, 0.3).translucent(), "0.3 shows the desktop through");
    }
}

#[test]
fn a_value_out_of_range_is_clamped() {
    assert_eq!(Glass::new(&DARK, -1.).chrome_alpha(), 1.);
    assert_eq!(Glass::new(&DARK, f32::NAN).chrome_alpha(), 1.);
    assert_eq!(Glass::new(&LIGHT, 5.).chrome_alpha(), Glass::new(&LIGHT, 0.8).chrome_alpha());
}
