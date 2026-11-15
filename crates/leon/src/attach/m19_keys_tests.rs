//! M19 criterion 15 (.agent/plans/2026-11-15-m19-tmux-migrants.md): `leon attach`'s repeat after
//! the prefix, and shifted symbols and capitals in `attach/` bindings.

use std::time::{Duration, Instant};

use super::*;
use leon_app::keymap::Repeat;
use serde_json::json;

fn key(c: char, mods: KeyModifiers) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), mods)
}

fn keys() -> Keys {
    let km = json!({
        "bindings": [
            { "key": "h", "mode": "attach", "action": "pane.focus.left", "repeats": true },
            { "key": "y", "mode": "attach", "action": "pane.zoom" },
            { "key": "ctrl+b>x", "action": "x_it", "repeats": true },
        ],
        "settings": { "repeat_time": 300 },
    });
    Keys::new(&km, None).unwrap()
}

#[test]
fn only_bindings_with_repeats_repeat() {
    let k = keys();
    let c = |s: &str| one_chord(s).unwrap();
    assert_eq!(k.repeatable(&c("h")), Some(After::Run("pane.focus.left".into(), None)));
    assert!(
        matches!(k.repeatable(&c("x")), Some(After::Config(Pressed::Run(b))) if b.action.as_deref() == Some("x_it")),
        "a prefix>KEY sequence with repeats"
    );
    assert_eq!(k.repeatable(&c("y")), None, "an attach/ binding without repeats");
    assert_eq!(k.repeatable(&c("d")), None, "the built-in table never repeats");
    assert_eq!(k.repeatable(&c("q")), None);
}

#[test]
fn the_window_after_the_prefix() {
    let k = keys();
    let (mut r, t0, ms) = (Repeat::default(), Instant::now(), Duration::from_millis);
    let prefix = [k.prefix.clone()];
    let h = one_chord("h").unwrap();
    assert_eq!(k.config.repeat_time, ms(300));
    r.ran(&prefix, true, k.config.repeat_time, t0);
    let hit = r.press(&h, t0 + ms(100), |_, c| k.repeatable(c));
    assert_eq!(hit.map(|(a, p)| (a, p.len())), Some((After::Run("pane.focus.left".into(), None), 1)));
    r.ran(&prefix, true, k.config.repeat_time, t0 + ms(100));
    assert!(r.press(&h, t0 + ms(401), |_, c| k.repeatable(c)).is_none(), "past the window");
}

#[test]
fn symbols_and_capitals_in_attach_bindings() {
    const SHIFTED: &str = "~!@#$%^&*()_+{}|:\"<>?";
    for sym in SHIFTED.chars() {
        let km = json!({ "bindings": [{ "key": sym.to_string(), "mode": "attach", "action": "pane.zoom" }] });
        let k = Keys::new(&km, None).unwrap();
        for mods in [KeyModifiers::NONE, KeyModifiers::SHIFT] {
            let c = chord(&key(sym, mods)).unwrap_or_else(|| panic!("{sym} has a name"));
            assert_eq!(k.after_prefix(&c), After::Run("pane.zoom".into(), None), "attach/{sym} with {mods:?}");
        }
    }
    let km = json!({ "bindings": [{ "key": "shift+f", "mode": "attach", "action": "client.hints" }] });
    let k = Keys::new(&km, None).unwrap();
    assert_eq!(
        k.after_prefix(&chord(&key('F', KeyModifiers::SHIFT)).unwrap()),
        After::Run("client.hints".into(), None)
    );
    assert_ne!(k.after_prefix(&chord(&key('f', KeyModifiers::NONE)).unwrap()), After::Run("client.hints".into(), None));
}
