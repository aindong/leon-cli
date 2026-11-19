//! GPUI phase 6: the ADR says where the overlays are (their focus and key contexts, which are
//! views and why none is cached), and every page of it stays under 20 KB.
mod help_walk;

fn doc(path: &str) -> String {
    std::fs::read_to_string(help_walk::repo().join(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
}

#[test]
fn the_adr_says_where_the_overlays_are() {
    let adr = doc("docs/architecture/gpui.md");
    let consequences = adr.split("\n## Consequences\n").nth(1).expect("a Consequences section");
    assert!(
        !consequences.contains("handled by hand inside one `key_down`"),
        "Consequences still say the overlays' keys go through one key_down"
    );
    for words in ["key context", "focus handle", "`Palette`", "`FindBar`", "`Overview`", "`RenameField`"] {
        assert!(consequences.contains(words), "Consequences name the overlays' {words}");
    }
    let r3 = adr
        .lines()
        .skip_while(|l| !l.starts_with("- **R3."))
        .take_while(|l| !l.starts_with("- **R4."))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(r3.contains("not cached"), "R3 says which overlays aren't cached and why: {r3}");
    for e in std::fs::read_dir(help_walk::repo().join("docs/architecture")).unwrap().flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with("gpui") && name.ends_with(".md") {
            let len = std::fs::metadata(e.path()).unwrap().len();
            assert!(len < 20_000, "docs/architecture/{name} stays under 20 KB: {len}");
        }
    }
}
