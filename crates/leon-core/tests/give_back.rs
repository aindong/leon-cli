//! A terminal's history, once freed, is handed back to the system: the allocator keeps freed pages
//! otherwise, and the server's footprint climbs between session churns. Closing many panes at
//! once gives back once or a few times, not once a pane; and every burst of closes gives back.
// macOS only: the give-back uses malloc_zone_pressure_relief, measured with the macOS soak
#![cfg(target_os = "macos")]
use std::time::{Duration, Instant};

use leon_core::terminal::{given_back, Spec, Terminal};

/// Drop `n` terminals with history at once; how many give-backs followed, once the count settles.
fn burst(n: usize) -> usize {
    let spawn = || {
        let command = vec!["/bin/sh".into(), "-c".into(), "seq 1 5000; exec sleep 100".into()];
        Terminal::spawn(Spec { command, cwd: None, env: vec![] }, |_| {}).unwrap()
    };
    let ts: Vec<Terminal> = (0..n).map(|_| spawn()).collect();
    let end = Instant::now() + Duration::from_secs(10);
    while !ts.iter().all(|t| t.text(true).contains("5000")) && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(20));
    }
    let before = given_back();
    drop(ts);
    // each program is hung up, its reader lets go of the emulator, and the history is freed; then
    // the count settles (no give-back for 1 s)
    let end = Instant::now() + Duration::from_secs(10);
    let (mut last, mut since) = (given_back(), Instant::now());
    while Instant::now() < end && (last == before || since.elapsed() < Duration::from_secs(1)) {
        std::thread::sleep(Duration::from_millis(20));
        if given_back() != last {
            (last, since) = (given_back(), Instant::now());
        }
    }
    given_back() - before
}

#[test]
fn every_burst_of_closes_gives_memory_back_together() {
    for round in 1..=2 {
        let gave = burst(8);
        assert!(gave >= 1, "round {round}: no memory was given back after the terminals were dropped");
        assert!(gave < 8, "round {round}: 8 closes gave back {gave} times: they should share give-backs");
    }
}
