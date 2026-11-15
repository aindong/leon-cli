//! Daily-driver F12 (.agent/plans/2026-11-15-cli-docs-polish.md, criterion 6): keys typed into
//! `leon -H … attach` while the link is stalled arrive whole or not at all. The stall is the fake
//! ssh's transports stopped with SIGSTOP: their connections stay open and nothing moves, as with a
//! paused machine. The research repro left `ec` of `echo during-pause⏎` at the remote prompt.

mod support;

use std::time::Duration;

use support::host::{assert_restored, attach_then_stty, Host};
use support::remote::Remote;

/// Signal every transport still running (`leon server proxy`, which each fake ssh became by exec);
/// returns their pids.
fn signal_transports(r: &Remote, sig: i32) -> Vec<i32> {
    let pids = std::fs::read_to_string(r.fake.path().join("pids")).unwrap_or_default();
    let mut hit = vec![];
    for pid in pids.lines().filter_map(|p| p.trim().parse::<i32>().ok()) {
        let out = std::process::Command::new("ps").args(["-o", "command=", "-p", &pid.to_string()]).output().unwrap();
        if String::from_utf8_lossy(&out.stdout).contains("server proxy") {
            // SAFETY: a signal to a process this test started
            unsafe { libc::kill(pid, sig) };
            hit.push(pid);
        }
    }
    hit
}

#[test]
fn keys_typed_into_a_stalled_link_never_arrive_in_part() {
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

    let stopped = signal_transports(&r, libc::SIGSTOP);
    assert!(stopped.len() >= 3, "attach runs over its transports");
    let before = r.ssh_calls().len();
    // the user stops to think, then types: the scenario's input, not a wait for output
    std::thread::sleep(Duration::from_millis(1500));
    h.send(b"echo during-pause\r");
    h.wait_for("the note that the keys weren't sent", |s| s.contains("not sent"));
    // the link comes back over new transports while the old ones are still stopped
    h.wait_for("the link to come back", |s| r.ssh_calls().len() >= before + 3 && !s.contains("reconnecting"));
    signal_transports(&r, libc::SIGCONT);

    h.send(b"echo back-$((6*7))\r");
    h.wait_text("back-42");
    let remote = r.ok(&["-H", "ssh://box", "capture", "-b", &b]);
    assert!(!remote.contains("during-pause"), "the stalled keys never ran:\n{remote}");
    let line = remote.lines().find(|l| l.contains("back-$((6*7))")).unwrap_or_else(|| panic!("{remote}"));
    assert!(line.contains("$ echo back-"), "no fragment of the stalled keys before the next command: {line:?}");
    h.send(b"\x02d");
    assert_restored(&h, 0);
}
