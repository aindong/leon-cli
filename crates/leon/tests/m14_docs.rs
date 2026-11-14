//! M14 acceptance (.agent/plans/2026-11-15-m14-resume-after-restart.md, criterion 10): the docs
//! record the scrollback caps and the resume policies, and tell agents about `set_resume`.

use std::path::Path;

fn doc(rel: &str) -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(rel)).unwrap()
}

#[test]
fn the_docs_cover_scrollback_and_resume() {
    let arch = doc("docs/architecture.md");
    for want in ["2 MiB", "8 MiB", "set_resume", "\"ask\"", "\"always\"", "\"never\""] {
        assert!(arch.contains(want), "docs/architecture.md names {want}");
    }
    let lua = doc("docs/lua.md");
    assert!(lua.contains("leon.set(\"resume\""), "docs/lua.md documents leon.set");
    for (file, text) in [("guide.md", doc("crates/leon/src/skill/guide.md")), ("agents.md", doc("docs/agents.md"))] {
        assert!(text.contains("set_resume"), "{file} tells agents about set_resume");
    }
    let cli = doc("docs/cli.md");
    assert!(cli.contains("resume"), "docs/cli.md mentions the resume command");
}
