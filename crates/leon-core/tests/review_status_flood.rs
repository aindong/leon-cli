//! Review proof: one PTY read of flapping OSC 7501 reports gives a bounded number of changes (the
//! terminal's reader coalesces each read's changes before they become events).

use leon_core::program_status::{coalesce, Tracker};

#[test]
fn one_read_of_flapping_reports_yields_a_bounded_number_of_changes() {
    let mut input = Vec::new();
    while input.len() < 8000 {
        input.extend_from_slice(b"\x1b]7501;state=idle\x07\x1b]7501;state=done\x07");
    }
    let changes = coalesce(Tracker::default().feed(&input, |_| {}).changes);
    assert!(changes.len() <= 16, "{} program_status events from {} PTY bytes", changes.len(), input.len());
}
