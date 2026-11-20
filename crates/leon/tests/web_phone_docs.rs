//! leon in the browser, PR 3 (.agent/plans/2026-11-20-web-phone-layout.md), criterion 7: the docs
//! say how the page works on a phone.

fn doc(path: &str) -> String {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::read_to_string(root.join(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
}

#[test]
fn the_phone_layout_is_documented() {
    let page = doc("docs/web.md");
    let at = page.find("## On a phone").expect("docs/web.md has an \"On a phone\" section");
    let phone = &page[at..];
    let phone = &phone[..phone[3..].find("\n## ").map_or(phone.len(), |e| e + 3)];
    for s in ["Activity", "‹", "☰", "key row", "Esc", "Ctrl", "Tab", "arrows", "keyboard", "Backspace"] {
        assert!(phone.contains(s), "the \"On a phone\" section says {s}");
    }
    assert!(phone.contains("emulation"), "it says how the phone layout was checked");
}
