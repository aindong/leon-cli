//! M22b criterion 2: `server.settings` resolved into the themes and font the app draws with.

use leon_app::look::Look;
use leon_app::theme::{DARK, LIGHT};
use serde_json::json;

fn settings(dark: &str, light: &str) -> serde_json::Value {
    json!({
        "theme": { "dark": dark, "light": light },
        "font": { "family": "Monaco", "size": 15.0 },
        "notifications": { "done": true, "blocked": true, "error": true },
        "copy_on_select": false,
    })
}

#[test]
fn a_pair_follows_the_appearance() {
    let dir = tempfile::tempdir().unwrap();
    let look = Look::resolve(&settings("leon-dark", "leon-light"), dir.path());
    assert_eq!(look.theme(true), DARK, "macOS dark");
    assert_eq!(look.theme(false), LIGHT, "macOS light");
    assert_eq!((look.font_family.as_str(), look.font_size), ("Monaco", 15.));
}

#[test]
fn a_stored_theme_is_read_from_its_file() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("harbor.theme"), "background = #102030\nforeground = #e0e0e0\n").unwrap();
    let look = Look::resolve(&settings("harbor", "harbor"), dir.path());
    let t = look.theme(true);
    assert_eq!((t.bg, t.fg), (0x102030, 0xe0e0e0));
    assert_eq!(look.theme(false), t, "one name serves both appearances");
    assert_ne!(t.chrome, DARK.chrome, "the chrome is mixed from the theme, not left dark");
}

#[test]
fn a_missing_theme_falls_back_by_appearance() {
    let dir = tempfile::tempdir().unwrap();
    let look = Look::resolve(&settings("gone", "gone-too"), dir.path());
    assert_eq!((look.theme(true), look.theme(false)), (DARK, LIGHT));
}

#[test]
fn no_settings_is_today() {
    let dir = tempfile::tempdir().unwrap();
    let look = Look::resolve(&json!({}), dir.path());
    assert_eq!((look.theme(true), look.theme(false)), (DARK, DARK), "leon-dark whatever the OS");
    assert_eq!((look.font_family.as_str(), look.font_size), ("Menlo", 13.));
}
