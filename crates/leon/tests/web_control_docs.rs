//! leon in the browser, PR 2 (.agent/plans/2026-11-20-web-full-control.md), criterion 6: the docs
//! say how the page is driven, and that it counts as a person.

fn doc(path: &str) -> String {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::read_to_string(root.join(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
}

#[test]
fn driving_the_page_is_documented() {
    let page = doc("docs/web.md");
    for s in ["Paste", "Copy", "mouse", "Activity", "notification", "split", "Close", "fits"] {
        assert!(page.to_lowercase().contains(&s.to_lowercase()), "docs/web.md: {s}");
    }
    assert!(!page.contains("view only") && !page.contains("view-only"), "docs/web.md no longer says view only");
    let arch = doc("docs/architecture/web.md");
    for s in ["Peer::Remote", "keys::encode", "block.event", "dev.leon.terminal.resize"] {
        assert!(arch.contains(s), "docs/architecture/web.md: {s}");
    }
}
