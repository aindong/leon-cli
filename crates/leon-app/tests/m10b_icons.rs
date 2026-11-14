//! M10b criterion 12: icons are real SVGs from one embedded set, at comfortable sizes, and no text
//! glyph stands in for one.

use gpui::AssetSource;
use leon_app::icons::{Icon, Icons, ROW_ICON, TOOLBAR_ICON, TOOLBAR_TARGET};

#[test]
fn every_icon_is_an_svg_in_the_set() {
    for icon in Icon::ALL {
        let bytes = Icons.load(icon.path()).unwrap().unwrap_or_else(|| panic!("{icon:?} is missing"));
        let svg = std::str::from_utf8(&bytes).unwrap();
        assert!(svg.starts_with("<svg") && svg.contains(r#"viewBox="0 0 24 24""#), "{icon:?}: one 24-unit grid");
        assert!(svg.contains(r#"stroke-width="1.75""#), "{icon:?}: one stroke weight");
        assert!(svg.contains(r#"stroke-linecap="round""#), "{icon:?}: round caps");
    }
    let listed = Icons.list("icons").unwrap();
    assert_eq!(listed.len(), Icon::ALL.len(), "the asset source lists each icon once");
}

#[test]
fn icons_are_a_comfortable_size() {
    assert!(TOOLBAR_ICON >= 16. && TOOLBAR_TARGET >= 28., "toolbar {TOOLBAR_ICON} in {TOOLBAR_TARGET}");
    assert!(ROW_ICON >= 14., "rows and pane headers: {ROW_ICON}");
}

#[test]
fn no_text_glyph_stands_in_for_an_icon() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for e in std::fs::read_dir(&src).unwrap() {
        let p = e.unwrap().path();
        if !p.file_name().unwrap().to_str().unwrap().starts_with("view") {
            continue;
        }
        let text = std::fs::read_to_string(&p).unwrap();
        for glyph in ["\"◧\"", "\"◫\"", "\"⊟\"", "\"⤢\"", "\"›_\"", "\"×\"", "\"+\"", "\"⌘\"", "\"›\""] {
            assert!(!text.contains(glyph), "{}: the glyph {glyph} stands in for an icon", p.display());
        }
    }
}
