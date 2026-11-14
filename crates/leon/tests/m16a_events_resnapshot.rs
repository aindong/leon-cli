//! M16a criterion 9 (.agent/plans/2026-11-15-m16a-remote-ssh.md): a link's reconnects never
//! start a server; when the server comes back new (the resume token is unknown), `events` prints
//! a fresh snapshot and goes on with the new server's events.

mod support;

use std::time::{Duration, Instant};

use serde_json::Value;
use support::remote::Remote;

#[test]
fn a_restarted_server_gets_a_new_snapshot() {
    let r = Remote::new();
    r.ok(&["-H", "ssh://box", "new", "one"]);
    let mut ev = r.events(&["-H", "ssh://box"]);
    r.remote.ok(&["--autostart=false", "server", "stop"]);
    r.until("the remote server to stop", || !r.remote.socket().exists());
    let end = Instant::now() + Duration::from_millis(1500);
    while Instant::now() < end {
        assert!(!r.remote.socket().exists(), "a reconnect started a server");
        std::thread::sleep(Duration::from_millis(50));
    }
    r.remote.ok(&["server", "start"]);
    let end = Instant::now() + Duration::from_secs(20);
    let snap: Value = loop {
        assert!(Instant::now() < end, "no new snapshot");
        if let Some(l) = ev.line(Duration::from_millis(500)) {
            let v: Value = serde_json::from_str(&l).unwrap();
            if v.get("snapshot").is_some() {
                break v;
            }
        }
    };
    ev.seq = snap["seq"].as_u64().unwrap();
    r.remote.ok(&["--autostart=false", "new", "two"]);
    let e = ev.until("session_created");
    assert_eq!(e.payload["session"]["label"], "two");
}
