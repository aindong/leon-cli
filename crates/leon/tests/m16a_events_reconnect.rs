//! M16a criterion 7 (.agent/plans/2026-11-15-m16a-remote-ssh.md): `leon -H … events` survives
//! the transport dying, twice, and resumes from its last seq: no event lost or repeated, no
//! second snapshot. `Stream::next` fails on any seq gap.

mod support;

use support::remote::Remote;

#[test]
fn events_resume_across_two_link_drops() {
    let r = Remote::new();
    r.ok(&["-H", "ssh://box", "new", "before"]);
    let mut ev = r.events(&["-H", "ssh://box", "--all"]);
    let mut made = vec![];
    for round in 1..=2 {
        assert!(r.kill_transports() >= 1, "round {round}: the events transport was running");
        for i in 1..=2 {
            let label = format!("down-{round}-{i}");
            r.remote.ok(&["--autostart=false", "new", &label]);
            made.push(label);
        }
        // the next drop only once this one has been resumed: wait for its events
        for label in &made[made.len() - 2..] {
            let e = ev.until("session_created");
            assert_eq!(e.payload["session"]["label"], label.as_str(), "{e:?}");
        }
    }
    r.remote.ok(&["--autostart=false", "new", "after"]);
    let e = ev.until("session_created");
    assert_eq!(e.payload["session"]["label"], "after");
    assert!(ev.child.try_wait().unwrap().is_none(), "events kept running");
}
