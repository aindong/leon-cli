//! M16a criterion 10 (.agent/plans/2026-11-15-m16a-remote-ssh.md): `leon -H … attach` shows and
//! types into a remote session, and survives the transport dying: once it has reconnected, a
//! line typed exactly once arrives.

mod support;

use support::host::{assert_restored, attach_then_stty, Host};
use support::remote::Remote;

fn proxies(r: &Remote) -> usize {
    let out = std::process::Command::new("ps").args(["-axo", "pid=,command="]).output().unwrap();
    let pids = std::fs::read_to_string(r.fake.path().join("pids")).unwrap_or_default();
    let ours: Vec<&str> = pids.lines().map(str::trim).collect();
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| l.contains("server proxy"))
        .filter(|l| l.split_whitespace().next().is_some_and(|p| ours.contains(&p)))
        .count()
}

#[test]
fn attach_over_ssh_survives_a_link_drop() {
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
    h.send(b"echo typed-$((6*7))\r");
    h.wait_text("typed-42");

    let before = std::fs::read_to_string(r.fake.path().join("pids")).unwrap().lines().count();
    assert!(r.kill_transports() >= 3, "attach runs over its transports");
    r.until("attach to open new transports", || {
        let now = std::fs::read_to_string(r.fake.path().join("pids")).unwrap().lines().count();
        now >= before + 3 && proxies(&r) >= 3
    });
    // every connection (events, model fetches, UI calls) is new: one try, no retry
    h.send(b"echo again-$((7*8))\r");
    h.wait_text("again-56");
    assert!(r.ok(&["-H", "ssh://box", "capture", "-b", &b]).contains("again-56"));

    h.send(b"\x02d");
    assert_restored(&h, 0);
}
