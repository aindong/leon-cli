//! A small read must not fan out into a large volume of events, even one read at a time.

use leon_core::program_status::{Tracker, MAX_RECORDS};

#[test]
fn a_small_read_cannot_fan_out_large_events() {
    let mut t = Tracker::default();
    let msg = "YWFh".repeat(682); // 2046 decoded bytes of 'a'
    t.feed(b"\x1b]7501;state=idle:app=a\x07", |_| {});
    for i in 0..MAX_RECORDS - 1 {
        t.feed(format!("\x1b]7501;state=working:id=c{i}:msg={msg}\x07").as_bytes(), |_| {});
    }
    let toggle = b"\x1b]7501;state=idle:app=b\x07";
    let fed = t.feed(toggle, |_| {});
    let out = serde_json::to_vec(&fed.changes).unwrap().len();
    assert!(out <= 64 * toggle.len(), "{} input bytes produced {} bytes of change events", toggle.len(), out);
}
