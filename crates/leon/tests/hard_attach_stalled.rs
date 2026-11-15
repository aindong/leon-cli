//! Audit A1 (the 0.1.0 app audit): `leon -H … attach` over a stalled link still reads the
//! detach key at once: a split pressed while the link is stalled queues off the input loop
//! instead of holding it for each call's deadline.

mod support;

use std::time::{Duration, Instant};

use support::host::{assert_restored, attach_then_stty, Host};
use support::remote::Remote;

/// Send `sig` to every transport (each fake ssh became `leon server proxy`); how many.
fn signal_transports(r: &Remote, sig: i32) -> usize {
    let pids = std::fs::read_to_string(r.fake.path().join("pids")).unwrap_or_default();
    let mut n = 0;
    for pid in pids.lines().filter_map(|p| p.trim().parse::<i32>().ok()) {
        let out = std::process::Command::new("ps").args(["-o", "command=", "-p", &pid.to_string()]).output().unwrap();
        if String::from_utf8_lossy(&out.stdout).contains("server proxy") {
            // SAFETY: a signal to a process this test started
            unsafe { libc::kill(pid, sig) };
            n += 1;
        }
    }
    n
}

#[test]
fn the_detach_key_works_over_a_stalled_link() {
    let r = Remote::new();
    r.ok(&["-H", "ssh://box", "new", "r1"]);
    let b = r.ok(&["-H", "ssh://box", "-s", "r1", "run", "--", "sh", "-c", "echo hello-remote; exec sh"]);
    r.until("the remote block's first line", || {
        r.ok(&["-H", "ssh://box", "capture", "-b", &b]).contains("hello-remote")
    });
    let vars = r.vars();
    let vars: Vec<(&str, &str)> = vars.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let h = Host::spawn(&r.local, &attach_then_stty("r1 -H ssh://box"), &vars);
    h.wait_text("hello-remote");

    assert!(signal_transports(&r, libc::SIGSTOP) >= 3, "attach runs over its transports");
    h.send(b"\x02%"); // split right: the pane's cwd, the split and a refetch, all over the link
    h.send(b"\x02\"");
    let t = Instant::now();
    h.send(b"\x02d");
    let s = h.wait_text("after-attach");
    let took = t.elapsed();
    signal_transports(&r, libc::SIGCONT);
    assert!(took < Duration::from_secs(3), "the detach key took {took:?} over a stalled link:\n{s}");
    assert_restored(&h, 0);
}
