//! Criterion 2: keystrokes and pastes become the bytes the PTY gets.

use gpui::{Keystroke, Modifiers};
use leon_app::input::key_bytes;
use leon_core::keys::paste_bytes;

fn ks(key: &str, key_char: Option<&str>, m: Modifiers) -> Keystroke {
    Keystroke { modifiers: m, key: key.into(), key_char: key_char.map(Into::into) }
}

fn plain(key: &str, key_char: Option<&str>) -> Option<Vec<u8>> {
    key_bytes(&ks(key, key_char, Modifiers::default()), false)
}

#[test]
fn printable_and_named_keys() {
    assert_eq!(plain("a", Some("a")).unwrap(), b"a");
    assert_eq!(key_bytes(&ks("a", Some("A"), Modifiers::shift()), false).unwrap(), b"A");
    assert_eq!(plain("enter", None).unwrap(), b"\r");
    assert_eq!(plain("backspace", None).unwrap(), b"\x7f");
    assert_eq!(plain("escape", None).unwrap(), b"\x1b");
    assert_eq!(plain("tab", None).unwrap(), b"\t");
    assert_eq!(key_bytes(&ks("tab", None, Modifiers::shift()), false).unwrap(), b"\x1b[Z");
    assert_eq!(plain("space", Some(" ")).unwrap(), b" ");
    assert_eq!(plain("delete", None).unwrap(), b"\x1b[3~");
    assert_eq!(plain("home", None).unwrap(), b"\x1b[H");
    assert_eq!(plain("f1", None).unwrap(), b"\x1bOP");
    assert_eq!(plain("pageup", None).unwrap(), b"\x1b[5~");
}

#[test]
fn control_and_arrows() {
    assert_eq!(key_bytes(&ks("c", None, Modifiers::control()), false).unwrap(), b"\x03");
    assert_eq!(plain("up", None).unwrap(), b"\x1b[A");
    assert_eq!(key_bytes(&ks("up", None, Modifiers::default()), true).unwrap(), b"\x1bOA");
    assert_eq!(key_bytes(&ks("left", None, Modifiers::control()), false).unwrap(), b"\x1b[1;5D");
    assert_eq!(key_bytes(&ks("left", None, Modifiers::alt()), false).unwrap(), b"\x1b[1;3D");
}

#[test]
fn option_composes_and_cmd_is_a_shortcut() {
    assert_eq!(key_bytes(&ks("s", Some("ß"), Modifiers::alt()), false).unwrap(), "ß".as_bytes());
    assert_eq!(key_bytes(&ks("t", None, Modifiers::command()), false), None);
    assert_eq!(key_bytes(&ks("left", None, Modifiers::command()), false), None);
}

#[test]
fn paste_turns_newlines_into_returns_and_brackets() {
    assert_eq!(paste_bytes("a\nb\r\nc", false), b"a\rb\rc");
    assert_eq!(paste_bytes("a\nb", true), b"\x1b[200~a\rb\x1b[201~");
    let evil = paste_bytes("x\x1b[201~rm -rf ~\n", true);
    assert_eq!(evil, b"\x1b[200~x[201~rm -rf ~\r\x1b[201~", "an ESC can't close the bracket early");
}
