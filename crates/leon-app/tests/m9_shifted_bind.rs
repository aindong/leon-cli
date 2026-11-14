//! A config binding written with a shifted symbol (docs/lua.md's `ctrl+b>%`) matches the typed key.
use gpui::{Keystroke, Modifiers};
use leon_app::keymap::{chord, Keymap, Pressed};
use serde_json::json;

#[test]
fn a_symbol_bound_as_typed_matches_the_typed_key() {
    let km = Keymap::new(&json!({ "generation": 1, "bindings": [
        { "key": "ctrl+b>%", "mode": null, "action": "pane.split", "args": { "direction": "right" } }
    ]}));
    let ctrl_b = chord(&Keystroke { modifiers: Modifiers::control(), key: "b".into(), key_char: None }).unwrap();
    assert_eq!(km.press(&[], &ctrl_b), Pressed::Pending);
    let pct =
        chord(&Keystroke { modifiers: Modifiers::default(), key: "%".into(), key_char: Some("%".into()) }).unwrap();
    assert!(matches!(km.press(&[ctrl_b], &pct), Pressed::Run(_)), "ctrl+b>% from docs/lua.md never matches a typed %");
}
