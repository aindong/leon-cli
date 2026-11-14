//! Criterion 14: single printable characters encode as themselves.
use leon_core::keys::encode;

#[test]
fn single_printable_characters_are_themselves() {
    for key in ["A", "Z", "+"] {
        assert_eq!(encode(key, false).as_deref(), Ok(key.as_bytes()), "key {key:?} must type itself");
    }
}
