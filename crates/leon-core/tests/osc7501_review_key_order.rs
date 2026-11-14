//! A long pair that is only malformed is skipped like any malformed pair; the key limit is for real keys.

use leon_core::program_status::{parse, Ignored, Message, State};

#[test]
fn long_malformed_key_is_skipped_long_real_key_discards() {
    assert!(matches!(
        parse(b"state=idle:Some Free Text Here=1"),
        Ok(Message::Set { record, .. }) if record.state == State::Idle
    ));
    assert_eq!(parse(b"state=idle:abcdefghijklmnopq=1"), Err(Ignored::TooLong("key")));
}
