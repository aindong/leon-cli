//! M22a review (skeptic): Ghostty strips one pair of double quotes around a config value, so a
//! theme with quoted colours loads in Ghostty; leon's importer reads it the same way.

use leon_core::theme::import::parse;

#[test]
fn quoted_ghostty_values_parse_like_ghostty() {
    let src = "background = \"#282a36\"\nforeground = \"#f8f8f2\"\ncursor-color = \"#f8f8f2\"\npalette = 1=#ff5555\n";
    let t = parse(src.as_bytes()).unwrap_or_else(|e| panic!("Ghostty loads this file, leon refused it: {e}"));
    assert_eq!(t.bg, 0x282a36);
    assert_eq!(t.fg, 0xf8f8f2);
    assert_eq!(t.cursor, 0xf8f8f2);
    assert_eq!(t.ansi[1], 0xff5555);
}
