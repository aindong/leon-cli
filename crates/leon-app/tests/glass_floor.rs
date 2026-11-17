//! A theme that is already low in contrast (an imported one), and a faded pane, are never made
//! harder to read than they are opaque, at any translucency.

use leon_app::fade;
use leon_app::glass::{worst_contrast, Glass};
use leon_app::theme::{contrast, DARK, LIGHT};
use leon_lua::settings::FADE;

/// Every pane of `window`, faded by `strength` or not, at every setting.
fn check(name: &str, window: &leon_app::theme::Theme, strength: f32) {
    for pane in [*window, fade::theme(window, strength)] {
        let opaque = contrast(pane.fg, pane.bg).min(4.5);
        for i in 0..=16 {
            let s = i as f32 * 0.05;
            let g = Glass::new(window, s, strength);
            let c = worst_contrast(pane.fg, &[(pane.bg, g.pane_alpha())]);
            assert!(c >= opaque - 0.01, "{name} faded {strength} at {s}: {c:.2} < {opaque:.2}");
        }
    }
}

#[test]
fn a_low_contrast_imported_theme_is_never_made_worse() {
    // grey on grey, 3:1 or so opaque, in a dark and a light window
    let mut dim = DARK;
    (dim.fg, dim.bg, dim.chrome, dim.text) = (0x8A8A8A, 0x303030, 0x3A3A3A, 0x8A8A8A);
    assert!(contrast(dim.fg, dim.bg) < 4.5);
    check("dim", &dim, 0.);
    check("dim", &dim, FADE as f32);
    let mut pale = LIGHT;
    (pale.fg, pale.bg, pale.chrome, pale.text) = (0x8C8C8C, 0xF4F4F4, 0xE8E8E8, 0x8C8C8C);
    assert!(contrast(pale.fg, pale.bg) < 4.5);
    check("pale", &pale, 0.);
    check("pale", &pale, FADE as f32);
}

#[test]
fn a_faded_pane_keeps_the_contrast_its_fade_gives_it() {
    for (name, t) in [("leon-dark", DARK), ("leon-light", LIGHT)] {
        check(name, &t, FADE as f32);
        check(name, &t, 0.8);
    }
}
