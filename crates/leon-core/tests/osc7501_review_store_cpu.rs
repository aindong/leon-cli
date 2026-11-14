use leon_core::program_status::{Tracker, MAX_RECORDS};
use std::time::{Duration, Instant};

#[test]
fn report_cost_does_not_scale_with_store_size() {
    let mut t = Tracker::default();
    let fill: String = (0..MAX_RECORDS).map(|i| format!("\x1b]7501;state=working:id=a/b/c/d/e/f/g/r{i}\x07")).collect();
    t.feed(fill.as_bytes(), |_| {});
    let burst: String = (0..3000)
        .map(|i| format!("\x1b]7501;state={}:id=a/b/c/d/e/f/g/r0\x07", if i % 2 == 0 { "blocked" } else { "working" }))
        .collect();
    let start = Instant::now();
    t.feed(burst.as_bytes(), |_| {});
    let took = start.elapsed();
    // Generous for debug builds on a loaded machine; the old per-report snapshot took seconds.
    assert!(took < Duration::from_millis(500), "{} bytes of reports took {took:?}", burst.len());
}
