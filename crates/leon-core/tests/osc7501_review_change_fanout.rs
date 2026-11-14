//! Toggling the root `app` must not multiply events by the number of descendants.

use leon_core::program_status::{Tracker, MAX_RECORDS};

#[test]
fn one_read_cannot_fan_out_unbounded_change_events() {
    let mut t = Tracker::default();
    let fill: String = (0..MAX_RECORDS - 1).map(|i| format!("\x1b]7501;state=working:id=c{i}\x07")).collect();
    t.feed(fill.as_bytes(), |_| {});
    let toggles: String = (0..1000)
        .map(|i| if i % 2 == 0 { "\x1b]7501;state=idle:app=a\x07" } else { "\x1b]7501;state=idle:app=b\x07" })
        .collect();
    let fed = t.feed(toggles.as_bytes(), |_| {});
    // One event per report, not one per descendant.
    assert!(fed.changes.len() <= 1000, "{} bytes of input produced {} change events", toggles.len(), fed.changes.len());
}
