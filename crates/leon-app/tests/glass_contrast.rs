//! Readability first: whatever is behind the window, text keeps its contrast. leon can't see
//! the desktop, so the worst case is checked: a pure white and a pure black backdrop.

use leon_app::fade;
use leon_app::glass::{worst_contrast, Glass};
use leon_app::theme::{DARK, LIGHT};
use leon_lua::settings::FADE;

#[test]
fn built_in_themes_keep_their_text_readable_at_every_setting() {
    for (name, t) in [("leon-dark", DARK), ("leon-light", LIGHT)] {
        for i in 0..=16 {
            let s = i as f32 * 0.05;
            let g = Glass::new(&t, s, FADE as f32);
            let chrome = (t.chrome, g.chrome_alpha());
            // one translucent fill per pixel: a pane is drawn straight over the desktop
            let body = worst_contrast(t.fg, &[(t.bg, g.pane_alpha())]);
            assert!(body >= 4.5, "{name} at {s}: body text on a pane {body:.2}");
            let text = worst_contrast(t.text, &[chrome]);
            assert!(text >= 4.5, "{name} at {s}: chrome text {text:.2}");
            let muted = worst_contrast(t.muted, &[chrome]);
            assert!(muted >= 3.0, "{name} at {s}: secondary text {muted:.2}");
            let f = fade::theme(&t, FADE as f32);
            let faded = worst_contrast(f.fg, &[(f.bg, g.pane_alpha())]);
            let opaque = leon_app::theme::contrast(f.fg, f.bg).min(4.5);
            assert!(faded >= opaque - 0.01, "{name} at {s}: a faded pane {faded:.2} < {opaque:.2}");
        }
    }
}

#[test]
fn worst_contrast_is_the_opaque_contrast_for_an_opaque_fill() {
    let c = leon_app::theme::contrast(DARK.fg, DARK.bg);
    assert!((worst_contrast(DARK.fg, &[(DARK.bg, 1.)]) - c).abs() < 1e-9);
    // a clear fill shows the backdrop: white text on it is 1:1 over white
    assert!((worst_contrast(0xFFFFFF, &[(DARK.bg, 0.)]) - 1.).abs() < 1e-9);
}
