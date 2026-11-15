//! M22a review (security): a theme file's text reaches the terminal through error messages, so
//! control characters (an OSC 52 clipboard write, a C1 CSI) are escaped, never echoed raw.

use leon_core::theme::import::parse;

#[test]
fn errors_never_echo_raw_control_characters() {
    for input in [
        "\u{1b}]0;pwned\u{7} = #000000\n".to_string(),
        "<?xml version=\"1.0\"?><plist><dict><key>Ansi \u{9b}31m Color</key><dict/></dict></plist>".to_string(),
    ] {
        let err = parse(input.as_bytes()).unwrap_err();
        assert!(!err.chars().any(char::is_control), "raw control character in {err:?}");
    }
}
