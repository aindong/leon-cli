//! M22a criterion 2 (.agent/plans/2026-11-15-m22a-themes-settings.md): Ghostty theme files (and
//! leon's own theme files, the same syntax) parse into a theme; malformed ones are clear errors.

use leon_core::theme::import::{parse, to_text};
use leon_core::theme::{DARK, LIGHT};

/// A theme written for this test, in Ghostty's syntax.
const HARBOR: &str = "# Harbor: a test theme
palette = 0=#101418
palette = 1=#e06c75
palette = 2=#98c379
palette = 3=#e5c07b
palette = 4=#61afef
palette = 5=#c678dd
palette = 6=#56b6c2
palette = 7=#abb2bf
palette = 8=#5c6370
palette = 9=#ef7c85
palette = 10=#a8d389
palette = 11=#f5d08b
palette = 12=#71bfff
palette = 13=#d688ed
palette = 14=#66c6d2
palette = 15=#ffffff
background = #14181c
foreground = c8ccd4
cursor-color = #ffcc00
cursor-text = #000000
selection-background = #3e4451
selection-foreground = #ffffff
palette = 200=#123456
";

#[test]
fn a_ghostty_theme_fills_the_terminal_palette_and_derives_the_rest() {
    let t = parse(HARBOR.as_bytes()).unwrap();
    assert_eq!(t.bg, 0x14181C);
    assert_eq!(t.fg, 0xC8CCD4, "a colour without # still parses");
    assert_eq!(t.cursor, 0xFFCC00);
    assert_eq!(t.selection, 0x3E4451);
    assert_eq!(t.ansi[1], 0xE06C75);
    assert_eq!(t.ansi[15], 0xFFFFFF);
    assert!(t.is_dark());
    assert_ne!(t.chrome, DARK.chrome, "chrome is mixed from the theme's own colours");
    assert_eq!((t.working, t.done, t.blocked, t.error), (DARK.working, DARK.done, DARK.blocked, DARK.error));
}

#[test]
fn a_light_background_takes_the_light_status_colours_and_missing_ansi() {
    let t = parse(b"background = #fafafa\nforeground = #202020\npalette = 1=#aa0000\n").unwrap();
    assert!(!t.is_dark());
    assert_eq!(t.working, LIGHT.working);
    assert_eq!(t.ansi[1], 0xAA0000);
    assert_eq!(t.ansi[4], LIGHT.ansi[4], "a colour the file doesn't give comes from the base");
    assert_eq!(t.cursor, 0x202020, "no cursor colour: the foreground");
}

#[test]
fn leon_theme_text_round_trips_every_token() {
    for t in [DARK, LIGHT, parse(HARBOR.as_bytes()).unwrap()] {
        assert_eq!(parse(to_text(&t).as_bytes()).unwrap(), t);
    }
}

#[test]
fn malformed_ghostty_files_are_clear_errors() {
    let big = format!("background = #000000\nforeground = #ffffff\n# {}\n", "x".repeat(1 << 20));
    let cases: &[(&[u8], &str)] = &[
        (b"", "no background"),
        (b"# only a comment\n", "no background"),
        (b"background = #00000g\nforeground = #ffffff\n", "line 1"),
        (b"background = #0000\nforeground = #ffffff\n", "line 1"),
        (b"background = #000000\nforeground = #ffffff\npalette = 300=#000000\n", "line 3"),
        (b"background = #000000\nforeground = #ffffff\npalette = x=#000000\n", "line 3"),
        (b"background = #000000\nforeground = #ffffff\npalette = 1\n", "line 3"),
        (b"background = #000000\nthis is not a setting\n", "line 2"),
        (b"background = #000000\nforeground = #ffffff\nfont-family = Menlo\n", "font-family"),
        (b"foreground = #ffffff\n", "no background"),
        (b"background = #000000\n", "no foreground"),
        (b"background = \nforeground = #ffffff\n", "line 1"),
        (b"\x00\x01\x02\xff\xfe", "text"),
        (big.as_bytes(), "1 MiB"),
    ];
    for (input, want) in cases {
        let r = std::panic::catch_unwind(|| parse(input));
        let shown = String::from_utf8_lossy(&input[..input.len().min(60)]).into_owned();
        let err = match r {
            Ok(Err(e)) => e,
            Ok(Ok(t)) => panic!("{shown:?} parsed into {t:?}"),
            Err(_) => panic!("{shown:?} panicked"),
        };
        assert!(err.contains(want), "{shown:?}: error {err:?} should mention {want:?}");
    }
}
