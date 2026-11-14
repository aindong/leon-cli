//! M11 criterion 10: the docs describe scrollback, selection, mouse reporting, find and links.

fn doc(path: &str) -> String {
    std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(path)).unwrap()
}

#[test]
fn the_docs_cover_m11() {
    let lua = doc("docs/lua.md");
    assert!(lua.contains("copy_on_select"), "lua.md documents the copy_on_select setting");
    let arch =
        ["docs/architecture.md", "docs/architecture/protocol-and-clients.md", "docs/architecture/state-and-hosts.md"]
            .map(doc)
            .concat();
    for topic in [
        "dev.leon.terminal.lines",
        "scrolled",
        "dev.leon.terminal.find",
        "dev.leon.terminal.selection",
        "dev.leon.terminal.link_at",
        "mouse",
    ] {
        assert!(arch.contains(topic), "architecture.md covers {topic}");
    }
    let readme = doc("README.md");
    for key in ["⌘C", "⌘A", "⌘F", "⇧PageUp", "⌘↑"] {
        assert!(readme.contains(key), "README lists {key}");
    }
}
