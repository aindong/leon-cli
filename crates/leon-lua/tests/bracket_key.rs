//! Spec §4: "Keys: a character". A lone `[` is a character, like `]` already is.

use leon_lua::keys::parse;

fn norm(s: &str) -> String {
    parse(s).unwrap_or_else(|e| panic!("{s:?}: {e}")).to_string()
}

#[test]
fn lone_open_bracket_is_a_character_key() {
    assert_eq!(norm("ctrl+]"), "ctrl+]");
    assert_eq!(norm("ctrl+b>["), "ctrl+b>[");
    assert_eq!(norm("ctrl+["), "ctrl+[");
    assert_eq!(norm("Shift+Cmd+["), "cmd+shift+[");
    assert_eq!(norm("cmd+[KeyA]"), "cmd+[KeyA]");
}

#[test]
fn a_bad_mode_name_is_named_as_such() {
    let e = parse("Bad Mode/x").unwrap_err();
    assert!(e.contains("mode name \"Bad Mode\""), "{e}");
    assert!(parse("[]").unwrap_err().contains("physical"));
    assert!(parse("[KeyA").unwrap_err().contains("physical"));
    assert!(parse("[>a").is_ok());
}
