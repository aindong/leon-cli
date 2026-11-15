//! Daily-driver criterion 5 (.agent/plans/2026-11-15-daily-driver-fixes.md): while the link to a
//! host is down, `leon -H … attach` says so in its status bar instead of freezing without a word,
//! and the note goes once the link is back.

mod support;

use support::host::{assert_restored, attach_then_stty, Host};
use support::remote::Remote;

#[test]
fn attach_says_it_is_reconnecting_while_the_link_is_down() {
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

    let down = r.fake.path().join("down");
    std::fs::write(&down, "").unwrap();
    assert!(r.kill_transports() >= 3, "attach runs over its transports");
    let s = h.wait_for("the reconnecting note", |s| s.contains("reconnecting to box"));
    assert!(s.contains("hello-remote"), "the last screen stays up:\n{s}");

    std::fs::remove_file(&down).unwrap();
    h.wait_for("the note to go", |s| !s.contains("reconnecting"));
    h.send(b"echo back-$((6*7))\r");
    h.wait_text("back-42");
    h.send(b"\x02d");
    assert_restored(&h, 0);
}

#[test]
fn the_picker_says_it_too() {
    let r = Remote::new();
    r.ok(&["-H", "ssh://box", "new", "r1"]);
    let vars = r.vars();
    let vars: Vec<(&str, &str)> = vars.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let h = Host::spawn(&r.local, &attach_then_stty("-H ssh://box"), &vars);
    h.wait_text("Pick a session");

    std::fs::write(r.fake.path().join("down"), "").unwrap();
    assert!(r.kill_transports() >= 3, "attach runs over its transports");
    h.wait_for("the reconnecting note over the picker", |s| s.contains("reconnecting to box"));
    std::fs::remove_file(r.fake.path().join("down")).unwrap();
    h.wait_for("the note to go", |s| !s.contains("reconnecting"));
    h.send(b"\x1b");
    assert_restored(&h, 0);
}
