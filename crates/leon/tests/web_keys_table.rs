//! leon in the browser, PR 2 (.agent/plans/2026-11-20-web-full-control.md), criterion 1: the page's
//! table of browser keys to binding names (crates/leon-server/web/keys.js) names only keys leon
//! encodes, with the modifier forms the page builds, so the server's mapping is the only one.

use leon_core::keys::encode;
use serde_json::Value;

/// keys.js's `NAMES`: the JSON object between `export const NAMES = ` and the `;` after it.
fn names() -> Value {
    let js =
        std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../leon-server/web/keys.js"))
            .expect("keys.js");
    let at = js.find("export const NAMES = ").expect("a NAMES table") + "export const NAMES = ".len();
    let end = js[at..].find("};").expect("its end") + 1;
    serde_json::from_str(&js[at..at + end]).expect("NAMES is JSON-shaped")
}

#[test]
fn every_name_the_page_sends_is_a_key_leon_encodes() {
    let table = names();
    let table = table.as_object().unwrap();
    for k in ["Enter", "Escape", "Tab", "Backspace", "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight", "Home", "End"] {
        assert!(table.contains_key(k), "NAMES has {k}");
    }
    for k in ["PageUp", "PageDown", "Insert", "Delete", "F1", "F5", "F12", " "] {
        assert!(table.contains_key(k), "NAMES has {k:?}");
    }
    for (dom, name) in table {
        let name = name.as_str().unwrap();
        for prefix in ["", "ctrl+", "alt+", "shift+", "ctrl+alt+", "ctrl+shift+"] {
            let full = format!("{prefix}{name}");
            assert!(encode(&full, false).is_ok(), "{dom:?} → {full:?} isn't a key leon encodes");
        }
    }
    for c in ('a'..='z').chain('0'..='9').chain("[]\\/-".chars()) {
        assert!(encode(&format!("alt+{c}"), false).is_ok(), "alt+{c}");
    }
    // the only characters keys.js sends with ctrl
    let js =
        std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../leon-server/web/keys.js"))
            .unwrap();
    let at = js.find("export const CTRL = ").expect("a CTRL list") + "export const CTRL = ".len();
    let end = js[at..].find(";").unwrap();
    let ctrl: String = serde_json::from_str(&js[at..at + end]).expect("CTRL is a JSON string");
    assert!(ctrl.contains('c') && ctrl.contains('['), "{ctrl}");
    for c in ctrl.chars() {
        assert!(encode(&format!("ctrl+{c}"), false).is_ok(), "ctrl+{c:?} is in CTRL but doesn't encode");
    }
    for c in "13457890=;',.`".chars() {
        assert!(!ctrl.contains(c), "ctrl+{c} has no control code, so keys.js mustn't send it");
    }
}
