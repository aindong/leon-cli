//! M16a criterion 3 (.agent/plans/2026-11-15-m16a-remote-ssh.md): `leon -H` commands act on the
//! remote server, never a local one, and ignore the local LEON_SESSION/LEON_BLOCK.

mod support;

use std::time::{Duration, Instant};

use support::remote::Remote;

const H: &str = "ssh://box";

#[test]
fn commands_act_on_the_remote_server() {
    let r = Remote::new();
    let run = |args: &[&str]| {
        let out = r.cmd().env("LEON_SESSION", "session:local-only").env("LEON_BLOCK", "block:local-only").args(args).output().unwrap();
        assert!(out.status.success(), "leon {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8(out.stdout).unwrap().trim().to_string()
    };
    let sid = run(&["-H", H, "new", "r1"]);
    assert!(sid.starts_with("session:"), "{sid}");
    let remote_ls = r.remote.json(&["--autostart=false", "ls", "--json"]);
    assert_eq!(remote_ls[0]["label"], "r1", "the remote server has the session: {remote_ls}");

    let b = run(&["-H", H, "-s", "r1", "run", "--", "sh", "-c", "echo hi-remote; exec sh"]);
    let capture = |what: &str| {
        let end = Instant::now() + Duration::from_secs(10);
        loop {
            let s = run(&["-H", H, "capture", "-b", &b]);
            if s.contains(what) {
                return s;
            }
            assert!(Instant::now() < end, "no {what} in:\n{s}");
            std::thread::sleep(Duration::from_millis(50));
        }
    };
    capture("hi-remote");
    run(&["-H", H, "send", "-b", &b, "echo sent-$((6*7))\r"]);
    capture("sent-42");
    // nothing names a session but the local env: the remote's only session is the one
    assert!(run(&["-H", H, "window", "ls"]).contains("window:"));
    assert!(run(&["-H", H, "ls"]).contains("r1"));
    assert!(run(&["-H", H, "api", "call", "session.list"]).contains("\"r1\""));

    let mut ev = r.events(&["-H", H, "--all"]);
    run(&["-H", H, "new", "r2"]);
    let e = ev.until("session_created");
    assert_eq!(e.payload["session"]["label"], "r2", "{e:?}");

    assert!(!r.local.socket().exists(), "no local server may start");
}
