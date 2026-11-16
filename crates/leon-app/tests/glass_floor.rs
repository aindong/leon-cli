//! A theme that is already low in contrast (an imported one), and a faded pane, are never made
//! harder to read than they are opaque, at any translucency.

use leon_app::fade;
use leon_app::glass::{worst_contrast, Glass};
use leon_app::theme::{contrast, DARK, LIGHT};
use leon_lua::settings::FADE;

fn check(name: &str, window: &leon_app::theme::Theme, pane: &leon_app::theme::Theme) {
    let opaque = contrast(pane.fg, pane.bg).min(4.5);
    for i in 0..=16 {
        let s = i as f32 * 0.05;
        let g = Glass::new(window, s);
        let c = worst_contrast(pane.fg, &[(pane.bg, g.pane_alpha(pane))]);
        assert!(c >= opaque - 0.01, "{name} at {s}: {c:.2} < {opaque:.2}");
    }
}

#[test]
fn a_low_contrast_imported_theme_is_never_made_worse() {
    // grey on grey, 3:1 or so opaque, in a dark and a light window
    let mut dim = DARK;
    (dim.fg, dim.bg, dim.chrome, dim.text) = (0x8A8A8A, 0x303030, 0x3A3A3A, 0x8A8A8A);
    assert!(contrast(dim.fg, dim.bg) < 4.5);
    check("dim", &dim, &dim);
    let mut pale = LIGHT;
    (pale.fg, pale.bg, pale.chrome, pale.text) = (0x8C8C8C, 0xF4F4F4, 0xE8E8E8, 0x8C8C8C);
    assert!(contrast(pale.fg, pale.bg) < 4.5);
    check("pale", &pale, &pale);
}

#[test]
fn a_faded_pane_keeps_the_contrast_its_fade_gives_it() {
    for (name, t) in [("leon-dark", DARK), ("leon-light", LIGHT)] {
        check(name, &t, &fade::theme(&t, FADE as f32));
        check(name, &t, &fade::theme(&t, 0.8));
    }
}
