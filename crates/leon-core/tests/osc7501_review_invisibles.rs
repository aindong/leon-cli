use base64::Engine as _;
use leon_core::program_status::{parse, Message};

#[test]
fn default_ignorable_chars_drop_the_title() {
    for c in ["\u{34f}", "\u{17b4}", "\u{17b5}", "\u{1bca0}", "\u{1d173}"] {
        let p = format!("state=done:title={}", base64::engine::general_purpose::STANDARD.encode(c));
        match parse(p.as_bytes()) {
            Ok(Message::Set { record, .. }) => assert!(record.title.is_none(), "invisible {c:?} kept"),
            other => panic!("unexpected {other:?}"),
        }
    }
}
