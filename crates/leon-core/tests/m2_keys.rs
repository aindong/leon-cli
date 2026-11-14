//! Key names in binding syntax become the bytes a terminal sends
//! (.agent/plans/2026-11-14-m2-terminal-blocks.md, criterion 14).

use leon_core::keys::encode;

#[test]
fn named_keys_and_modifiers() {
    let cases: &[(&str, &[u8])] = &[
        ("enter", b"\r"),
        ("escape", b"\x1b"),
        ("tab", b"\t"),
        ("backspace", b"\x7f"),
        ("ctrl+c", b"\x03"),
        ("CTRL+C", b"\x03"),
        ("up", b"\x1b[A"),
        ("down", b"\x1b[B"),
        ("right", b"\x1b[C"),
        ("left", b"\x1b[D"),
        ("alt+x", b"\x1bx"),
        ("a", b"a"),
        ("space", b" "),
    ];
    for (key, bytes) in cases {
        assert_eq!(encode(key, false).as_deref(), Ok(*bytes), "{key}");
    }
}

#[test]
fn arrows_follow_application_cursor_mode() {
    assert_eq!(encode("up", true).unwrap(), b"\x1bOA");
    assert_eq!(encode("left", true).unwrap(), b"\x1bOD");
}

#[test]
fn unknown_keys_are_errors() {
    for key in ["nosuchkey", "ctrl+", "", "ctrl+nosuchkey"] {
        let e = encode(key, false).unwrap_err();
        assert!(e.contains(key) || key.is_empty(), "{key}: {e}");
    }
}
