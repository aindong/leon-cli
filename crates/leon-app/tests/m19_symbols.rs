//! M19 criterion 4 (.agent/plans/2026-11-15-m19-tmux-migrants.md): every US shifted symbol,
//! written as typed in a binding, loads and matches the typed key in the app. (`leon attach`'s
//! side is in crates/leon/src/attach/keys_tests.rs.)

use gpui::{Keystroke, Modifiers};
use leon_app::keymap::{chord, Keymap, Pressed};
use serde_json::json;

const SHIFTED: &str = "~!@#$%^&*()_+{}|:\"<>?";
const BASES: &str = "`1234567890-=[]\\;',./";

fn ks(key: &str, modifiers: Modifiers) -> Keystroke {
    Keystroke { modifiers, key: key.into(), key_char: Some(key.into()) }
}

#[test]
fn every_shifted_symbol_loads_from_init_lua() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("init.lua");
    let src: String = SHIFTED.chars().map(|c| format!("leon.bind([=[ctrl+b>{c}]=], \"pane.zoom\")\n")).collect();
    let src = src + "leon.bind([=[attach/&]=], \"pane.close\")\nleon.bind([=[ctrl+|]=], \"pane.zoom\")\n";
    std::fs::write(&path, &src).unwrap();
    let c = leon_lua::load(&path).unwrap();
    assert!(c.diagnostics.is_empty(), "{:?}", c.diagnostics);
    assert_eq!(c.bindings.len(), SHIFTED.len() + 2);
}

#[test]
fn a_symbol_matches_as_typed_or_as_shift_and_its_key() {
    for (sym, base) in SHIFTED.chars().zip(BASES.chars()) {
        let km = Keymap::new(&json!({ "bindings": [
            { "key": format!("ctrl+b>{sym}"), "action": "pane.zoom" },
            { "key": format!("ctrl+{sym}"), "action": "pane.close" },
        ]}));
        let ctrl_b = chord(&ks("b", Modifiers::control())).unwrap();
        assert_eq!(km.press(&[], &ctrl_b), Pressed::Pending);
        let typed = chord(&ks(&sym.to_string(), Modifiers::none())).unwrap();
        let shifted = chord(&ks(&base.to_string(), Modifiers::shift())).unwrap();
        // macOS may also report the symbol with shift still held
        let both = chord(&ks(&sym.to_string(), Modifiers::shift())).unwrap();
        for c in [&typed, &shifted, &both] {
            let hit = km.press(std::slice::from_ref(&ctrl_b), c);
            assert!(matches!(hit, Pressed::Run(_)), "ctrl+b>{sym} didn't match {c}");
        }
        let ctrl_sym = chord(&ks(&sym.to_string(), Modifiers::control())).unwrap();
        assert!(matches!(km.press(&[], &ctrl_sym), Pressed::Run(_)), "ctrl+{sym} didn't match");
    }
}

#[test]
fn a_capital_is_written_shift_and_its_letter() {
    let km = Keymap::new(&json!({ "bindings": [{ "key": "ctrl+b>shift+f", "action": "client.hints" }] }));
    let ctrl_b = chord(&ks("b", Modifiers::control())).unwrap();
    for typed in [ks("F", Modifiers::shift()), ks("f", Modifiers::shift())] {
        let c = chord(&typed).unwrap();
        assert!(matches!(km.press(std::slice::from_ref(&ctrl_b), &c), Pressed::Run(_)), "⇧F as {:?}", typed.key);
    }
    let lower = chord(&ks("f", Modifiers::none())).unwrap();
    assert_eq!(km.press(std::slice::from_ref(&ctrl_b), &lower), Pressed::Unbound, "f alone is another key");
}
