//! GPUI phase 7: the ADR says how composed text reaches a pane (the workspace's input handler,
//! marked text at the cursor, press-and-hold off), and every page of it stays under 20 KB.
mod help_walk;

fn doc(path: &str) -> String {
    std::fs::read_to_string(help_walk::repo().join(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
}

#[test]
fn the_adr_says_how_composed_text_arrives() {
    let adr = doc("docs/architecture/gpui.md");
    let consequences = adr.split("\n## Consequences\n").nth(1).expect("a Consequences section");
    assert!(!consequences.contains("No `InputHandler`"), "Consequences still say there is no InputHandler");
    for words in ["input handler", "marked text", "press-and-hold"] {
        assert!(consequences.contains(words), "Consequences say: {words}");
    }
    for e in std::fs::read_dir(help_walk::repo().join("docs/architecture")).unwrap().flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with("gpui") && name.ends_with(".md") {
            let len = std::fs::metadata(e.path()).unwrap().len();
            assert!(len < 20_000, "{name} is {len} bytes, over the 20 KB cap");
        }
    }
}
