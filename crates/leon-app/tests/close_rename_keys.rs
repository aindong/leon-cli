//! The close shortcuts are the app's defaults, clash with none of them, show in the palette and
//! are in docs/shortcuts.md.
use std::collections::HashSet;

use leon_app::keymap::DEFAULTS;
use leon_app::palette::BUILTINS;

#[test]
fn the_defaults_and_the_palette() {
    assert!(DEFAULTS.contains(&("cmd+alt+w", "client.tab.close")));
    assert!(DEFAULTS.contains(&("cmd+alt+shift+w", "session.close")));
    let keys: Vec<&str> = DEFAULTS.iter().map(|(k, _)| *k).collect();
    assert_eq!(keys.len(), keys.iter().collect::<HashSet<_>>().len(), "no key bound twice: {keys:?}");
    for (name, title) in [
        ("client.tab.close", "Close Tab"),
        ("session.close", "Close Session"),
        ("client.tab.rename", "Rename Tab…"),
        ("session.rename", "Rename Session…"),
    ] {
        assert!(BUILTINS.contains(&(name, title)), "{name} in the palette as {title}");
    }
}

#[test]
fn the_shortcuts_doc_lists_them() {
    let doc = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/shortcuts.md")).unwrap();
    assert!(doc.contains("| ⌘⌥W | Close the tab"), "⌥⌘W row");
    assert!(doc.contains("| ⌘⌥⇧W | Close the session"), "⌥⇧⌘W row");
}
