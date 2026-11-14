//! M17 criterion 12: the docs describe `leon attach` where readers look for it. The cli-doc test
//! (m7b_cli_doc.rs) already holds docs/cli.md to `--help`; this checks the rest says the same.

mod help_walk;

fn doc(path: &str) -> String {
    let text = std::fs::read_to_string(help_walk::repo().join(path)).unwrap();
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn docs_cover_attach() {
    let cli = doc("docs/cli.md");
    assert!(cli.contains("### `leon attach [TARGET]`"), "docs/cli.md has a section for attach");
    let readme = doc("README.md");
    assert!(readme.contains("leon attach"), "README shows leon attach");
    for key in ["ctrl+b d", "ctrl+b %", "ctrl+b \"", "ctrl+b z", "ctrl+b c", "ctrl+b s"] {
        assert!(readme.contains(key), "README lists {key}");
    }
    let arch = doc("docs/architecture.md");
    let part = arch.split("**The terminal client.**").nth(1).expect("architecture.md has a terminal-client decision");
    for word in ["block.watch", "ratatui", "SIGHUP", "own size", "prefix"] {
        assert!(part.contains(word), "architecture.md's terminal-client paragraph mentions {word}");
    }
    let lua = doc("docs/lua.md");
    assert!(lua.contains("leon.mode(\"attach\", { prefix ="), "docs/lua.md shows the attach prefix");
    assert!(lua.contains("attach/"), "docs/lua.md shows attach/ bindings");
}
