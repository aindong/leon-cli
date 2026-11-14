//! An id with bytes outside the value grammar is a bad id, not a skipped pair that falls back to root.

use leon_core::program_status::{parse, Ignored};

#[test]
fn id_outside_the_value_grammar_is_a_bad_id() {
    for payload in
        ["state=working:id=a@b", "state=working:id=my agent", "state=working:id=\u{e9}", "state=clear:id=a@b"]
    {
        assert_eq!(parse(payload.as_bytes()), Err(Ignored::BadId), "{payload:?} must not reach the root");
    }
}
