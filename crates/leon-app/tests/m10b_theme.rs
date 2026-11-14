//! M10b criterion 4: the app's colours are leon's brand palette, from one theme module.

use leon_app::status::Indicator;
use leon_app::theme::{contrast, DARK};

#[test]
fn the_tokens_are_the_brand_palette() {
    let t = &DARK;
    assert_eq!(t.bg, 0x1F1C19, "panes are Coal");
    assert_eq!(t.chrome, 0x292521, "the window is Iron");
    assert_eq!(t.title, 0xF5F0E7, "titles are Parchment");
    assert_eq!(t.text, 0xD8CEC2, "body text is Chalk");
    assert_eq!(t.accent, 0xF3A747, "focus and cursor are Amber");
    assert_eq!(t.selected, 0x542D35, "the selected row is the Maroon tile");
    assert_eq!(t.brand, 0xB64C32, "Oxide");
    assert_eq!((t.working, t.done, t.blocked, t.error), (0xE78155, 0xB8C6AC, 0xE8C77D, 0xE28E9B));
    assert_eq!(t.cursor, t.accent);
}

#[test]
fn each_state_has_its_status_colour() {
    let t = &DARK;
    assert_eq!(Indicator::Working { progress: None }.color(t), t.working);
    assert_eq!(Indicator::Blocked(None).color(t), t.blocked);
    assert_eq!(Indicator::Done.color(t), t.done);
    assert_eq!(Indicator::Error.color(t), t.error);
}

#[test]
fn text_stays_readable() {
    let t = &DARK;
    assert!((contrast(0xD8CEC2, 0x292521) - 9.80).abs() < 0.05, "matches brand/README's Chalk/Iron");
    for (name, fg, bg) in [
        ("muted on Iron", t.muted, t.chrome),
        ("text on Iron", t.text, t.chrome),
        ("title on the selected row", t.title, t.selected),
        ("muted on the selected row", t.muted, t.selected),
        ("text on Coal", t.fg, t.bg),
        ("muted on hover", t.muted, t.hover),
    ] {
        let c = contrast(fg, bg);
        assert!(c >= 4.5, "{name}: {c:.2}:1 is under 4.5:1");
    }
}

/// No colour literal outside the theme: a 24-bit hex, or a colour built or named in place.
#[test]
fn no_hard_coded_colours_in_the_app() {
    let mut dirs = vec![std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src")];
    while let Some(dir) = dirs.pop() {
        for e in std::fs::read_dir(&dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                dirs.push(p); // a module's own folder (src/remote/)
                continue;
            }
            let text = std::fs::read_to_string(&p).unwrap();
            for (i, line) in text.lines().enumerate() {
                let hex = line.match_indices("0x").any(|(at, _)| {
                    let digits: String = line[at + 2..].chars().take_while(|c| c.is_ascii_hexdigit()).collect();
                    digits.len() == 6
                });
                let named =
                    ["hsla(", "rgba(", "Rgba {", "white()", "black()", "rgb(0x"].iter().any(|n| line.contains(n));
                assert!(!named && !hex, "{}:{}: a colour literal: {line}", p.display(), i + 1);
            }
        }
    }
}
