//! M16a criterion 5 (.agent/plans/2026-11-15-m16a-remote-ssh.md): the proxy starts a persistent
//! server on the remote (it outlives its idle timeout); `server status|stop` work over -H and
//! never start one.

mod support;

use std::time::{Duration, Instant};

use support::remote::Remote;

#[test]
fn the_remote_server_is_persistent_and_status_stop_work() {
    let r = Remote::new();
    let out = r.cmd().env("LEON_IDLE_EXIT_SECS", "1").args(["-H", "ssh://box", "ls"]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let st = r.remote.json(&["--autostart=false", "server", "status", "--json"]);
    assert_eq!(st["autostarted"], false, "{st}");
    let end = Instant::now() + Duration::from_secs(3);
    while Instant::now() < end {
        assert_eq!(r.remote.code(&["--autostart=false", "server", "status"]), 0, "the remote server idled out");
        std::thread::sleep(Duration::from_millis(250));
    }

    assert!(r.ok(&["-H", "ssh://box", "server", "status"]).contains("running"));
    r.ok(&["-H", "ssh://box", "server", "stop"]);
    r.until("the remote server to stop", || r.remote.code(&["--autostart=false", "server", "status"]) == 4);

    assert_eq!(r.code(&["-H", "ssh://box", "server", "status"]), 4);
    assert!(!r.remote.socket().exists(), "status must not start a server");
}
