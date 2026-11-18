//! M14 acceptance (.agent/plans/2026-11-15-m14-resume-after-restart.md, criterion 10): the docs
//! record the scrollback caps and the resume policies, and tell agents about `set_resume`.

use std::path::Path;

fn doc(rel: &str) -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(rel)).unwrap()
}

#[test]
fn the_docs_cover_scrollback_and_resume() {
    let arch =
        ["docs/architecture.md", "docs/architecture/protocol-and-clients.md", "docs/architecture/state-and-hosts.md"]
            .map(doc)
            .concat();
    for want in ["2 MiB", "8 MiB", "set_resume", "\"ask\"", "\"always\"", "\"never\""] {
        assert!(arch.contains(want), "docs/architecture.md names {want}");
    }
    let lua = doc("docs/lua/settings.md");
    assert!(lua.contains("leon.set(\"resume\""), "docs/lua/settings.md documents leon.set");
    for (file, text) in [("guide.md", doc("crates/leon/src/skill/guide.md")), ("agents.md", doc("docs/agents.md"))] {
        assert!(text.contains("set_resume"), "{file} tells agents about set_resume");
    }
    // `leon skill` moved to docs/cli/agents.md (file-size limit)
    let cli = doc("docs/cli.md") + &doc("docs/cli/agents.md");
    for want in ["skill hook --agent", "set_resume", "resume command"] {
        assert!(cli.contains(want), "docs/cli.md mentions {want}");
    }
}
