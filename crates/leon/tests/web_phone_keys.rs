//! leon in the browser, PR 3 (.agent/plans/2026-11-20-web-phone-layout.md), criterion 1: the phone
//! key row (keys.js's `ROW`) sends only keys leon encodes, and its sticky Ctrl only `ctrl+` forms
//! leon encodes: with a named key, with a letter, or with a character in `CTRL`.

use leon_core::keys::encode;
use serde_json::Value;

fn keys_js() -> String {
    std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../leon-server/web/keys.js"))
        .expect("keys.js")
}

/// The JSON value between `export const <name> = ` and the `;` that ends it.
fn constant(js: &str, name: &str) -> Value {
    let start = format!("export const {name} = ");
    let at = js.find(&start).unwrap_or_else(|| panic!("keys.js has {name}")) + start.len();
    let end = js[at..].find(";\n").expect("its end");
    serde_json::from_str(&js[at..at + end]).unwrap_or_else(|e| panic!("{name} is JSON-shaped: {e}"))
}

#[test]
fn the_key_row_sends_only_keys_leon_encodes() {
    let js = keys_js();
    let row = constant(&js, "ROW");
    let ctrl = constant(&js, "CTRL");
    let ctrl = ctrl.as_str().unwrap();
    let row = row.as_array().expect("ROW is a list");
    let label = |e: &Value| e["label"].as_str().unwrap_or_default().to_string();
    let labels: Vec<String> = row.iter().map(label).collect();
    for want in ["Esc", "Ctrl", "Tab", "←", "↑", "↓", "→"] {
        assert!(labels.iter().any(|l| l == want), "the row has {want}: {labels:?}");
    }
    assert_eq!(row.iter().filter(|e| e["ctrl"] == true).count(), 1, "one sticky Ctrl");
    for e in row {
        match (e["key"].as_str(), e["text"].as_str()) {
            (Some(k), None) => {
                assert!(encode(k, false).is_ok(), "{} sends {k:?}, which leon doesn't encode", label(e));
                // the sticky Ctrl may come before it
                assert!(encode(&format!("ctrl+{k}"), false).is_ok(), "ctrl+{k} doesn't encode");
            }
            (None, Some(t)) => {
                assert_eq!(t.chars().count(), 1, "{} types one character", label(e));
                let c = t.chars().next().unwrap();
                if ctrl.contains(c) {
                    assert!(encode(&format!("ctrl+{c}"), false).is_ok(), "ctrl+{c}");
                }
            }
            _ => assert!(e["ctrl"] == true, "{} is a key, a character, or the Ctrl: {e}", label(e)),
        }
    }
    for c in 'a'..='z' {
        assert!(ctrl.contains(c), "CTRL has {c}, for the sticky Ctrl and a phone's keyboard");
        assert!(encode(&format!("ctrl+{c}"), false).is_ok(), "ctrl+{c}");
    }
}
