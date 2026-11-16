//! A terminal's history, once freed, is handed back to the system: the allocator keeps freed pages
//! otherwise, and the server's footprint climbs between session churns.
use std::time::{Duration, Instant};

use leon_core::terminal::{given_back, Spec, Terminal};

#[test]
fn a_dropped_terminal_gives_its_memory_back() {
    let spec = Spec {
        command: vec!["/bin/sh".into(), "-c".into(), "seq 1 5000; exec sleep 100".into()],
        cwd: None,
        env: vec![],
    };
    let t = Terminal::spawn(spec, |_| {}).unwrap();
    let end = Instant::now() + Duration::from_secs(10);
    while !t.text(true).contains("5000") && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(20));
    }
    let before = given_back();
    drop(t);
    // the program is hung up, its reader lets go of the emulator, and the history is freed
    let end = Instant::now() + Duration::from_secs(10);
    while given_back() == before && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(given_back() > before, "no memory was given back after the terminal was dropped");
}
