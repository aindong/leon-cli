use base64::Engine as _;
use leon_core::program_status::{parse, Message};

#[test]
fn default_ignorable_characters_drop_the_field() {
    for text in ["a\u{1d173}b", "a\u{1bca0}b", "a\u{e0100}b", "a\u{034f}b", "a\u{180b}b", "\u{200d}"] {
        let payload = format!("state=done:title={}", base64::engine::general_purpose::STANDARD.encode(text));
        match parse(payload.as_bytes()) {
            Ok(Message::Set { record, .. }) => assert_eq!(record.title, None, "{text:?} kept"),
            other => panic!("unexpected {other:?}"),
        }
    }
}

#[test]
fn joiners_and_selectors_only_follow_something_visible() {
    for text in ["\u{fe0f}x", "a\u{200d}\u{200d}b", "a\u{200d}\u{fe0f}", "a\u{fe0f}\u{fe0f}", " \u{200d}", "   "] {
        let payload = format!("state=done:msg={}", base64::engine::general_purpose::STANDARD.encode(text));
        match parse(payload.as_bytes()) {
            Ok(Message::Set { record, .. }) => assert_eq!(record.msg, None, "{text:?} kept"),
            other => panic!("unexpected {other:?}"),
        }
    }
    for text in ["🏳\u{fe0f}\u{200d}🌈", "❤\u{fe0f}\u{200d}🔥", "1\u{fe0f}\u{20e3}", "✌\u{fe0e}"] {
        let payload = format!("state=done:msg={}", base64::engine::general_purpose::STANDARD.encode(text));
        match parse(payload.as_bytes()) {
            Ok(Message::Set { record, .. }) => assert_eq!(record.msg.as_deref(), Some(text), "{text:?} dropped"),
            other => panic!("unexpected {other:?}"),
        }
    }
}
