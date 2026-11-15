//! M5a criterion 1 (.agent/plans/2026-11-14-m5a-lua-config.md): the key syntax of docs/lua.md.

use leon_lua::keys::parse;

fn norm(s: &str) -> String {
    parse(s).unwrap_or_else(|e| panic!("{s:?}: {e}")).to_string()
}

fn err(s: &str) -> String {
    match parse(s) {
        Ok(k) => panic!("{s:?} parsed as {k}"),
        Err(e) => e,
    }
}

#[test]
fn modifiers_are_case_and_order_insensitive_with_aliases() {
    assert_eq!(norm("Shift+Command+K"), "cmd+shift+k");
    assert_eq!(parse("shift+cmd+k"), parse("CMD+SHIFT+k"));
    for m in ["cmd", "command", "super", "meta", "win"] {
        assert_eq!(norm(&format!("{m}+a")), "cmd+a");
    }
    for m in ["ctrl", "control"] {
        assert_eq!(norm(&format!("{m}+a")), "ctrl+a");
    }
    for m in ["alt", "opt", "option"] {
        assert_eq!(norm(&format!("{m}+a")), "alt+a");
    }
    assert_eq!(norm("shift+alt+ctrl+cmd+x"), "cmd+ctrl+alt+shift+x");
}

#[test]
fn named_physical_and_punctuation_keys() {
    for k in [
        "enter",
        "escape",
        "tab",
        "space",
        "backspace",
        "delete",
        "left",
        "right",
        "up",
        "down",
        "home",
        "end",
        "pageup",
        "pagedown",
        "f1",
        "f12",
    ] {
        assert_eq!(norm(&k.to_uppercase()), k);
    }
    assert_eq!(norm("ctrl+Enter"), "ctrl+enter");
    assert_eq!(norm("cmd+[KeyA]"), "cmd+[KeyA]");
    assert_eq!(norm("[Digit1]"), "[Digit1]");
    assert_eq!(norm("cmd+shift+="), "cmd+shift+=");
    assert_eq!(norm("ctrl++"), "ctrl++");
    assert_eq!(norm("ctrl+/"), "ctrl+/");
    assert_eq!(norm("7"), "7");
}

#[test]
fn sequences_and_modes() {
    let k = parse("Ctrl+B>C").unwrap();
    assert_eq!(k.to_string(), "ctrl+b>c");
    assert_eq!(k.mode, None);
    assert_eq!(k.seq.len(), 2);
    assert_eq!(norm("ctrl+b>>"), "ctrl+b>>");
    assert_eq!(norm("g>g>g"), "g>g>g");

    let m = parse("resize/g>g").unwrap();
    assert_eq!(m.mode.as_deref(), Some("resize"));
    assert_eq!(m.to_string(), "resize/g>g");
    assert_eq!(norm("my_mode-2/Shift+Left"), "my_mode-2/shift+left");
    assert_ne!(parse("resize/left"), parse("left"));
}

#[test]
fn bad_keys_say_what_is_wrong() {
    assert!(err("").contains("empty"), "{}", err(""));
    assert!(err("ctrl+").contains("missing key"), "{}", err("ctrl+"));
    assert!(err("ctrl").contains("missing key"), "{}", err("ctrl"));
    assert!(err("ctrl+foo").contains("unknown key \"foo\""), "{}", err("ctrl+foo"));
    assert!(err("hyper+a").contains("unknown modifier \"hyper\""), "{}", err("hyper+a"));
    assert!(err("ctrl+control+a").contains("twice"), "{}", err("ctrl+control+a"));
    assert!(err("a>").contains("missing key"), "{}", err("a>"));
    assert!(err("Resize/x").contains("mode name"), "{}", err("Resize/x"));
    assert!(err("resize/").contains("missing key"), "{}", err("resize/"));
    assert!(err("[]").contains("physical"), "{}", err("[]"));
    assert!(err("[KeyA").contains("physical"), "{}", err("[KeyA"));
    assert!(err("a b").contains("unexpected"), "{}", err("a b"));
}
