//! ZWNJ is ordinary in Persian and Indic text; like ZWJ it may follow a visible character.

use base64::Engine as _;
use leon_core::program_status::{parse, Message};

fn title(s: &str) -> Option<String> {
    let p = format!("state=done:title={}", base64::engine::general_purpose::STANDARD.encode(s));
    match parse(p.as_bytes()) {
        Ok(Message::Set { record, .. }) => record.title,
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn persian_zwnj_title_is_kept() {
    let s = "می\u{200c}خواهم";
    assert_eq!(title(s).as_deref(), Some(s), "ZWNJ title dropped");
}

#[test]
fn joiners_need_a_visible_character_before_them() {
    for s in ["\u{200c}x", "a\u{200c}\u{200c}b", " \u{fe0f}", " \u{200d}", "a \u{200c}"] {
        assert_eq!(title(s), None, "{s:?} kept");
    }
}
