//! M16a impact: `-H` starts the link's threads before `leon attach` blocks its exit signals and
//! takes them with sigwait. Those threads block the signals themselves, so SIGTERM and SIGHUP
//! still reach attach's handler and the host terminal comes back cooked; and ssh, spawned from
//! those threads, starts with no signal blocked (Rust's Command resets the mask).

mod support;

use support::host::{assert_restored, attach_then_stty, Host};
use support::remote::Remote;

fn killed_by(signal: i32) {
    let r = Remote::new();
    r.ok(&["-H", "ssh://box", "new", "sig"]);
    let vars = r.vars();
    let vars: Vec<(&str, &str)> = vars.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let h = Host::spawn(&r.local, &attach_then_stty("sig -H ssh://box"), &vars);
    h.wait_for("attached", |_| h.alt_screen() && h.last_row().contains("sig"));
    let attach = std::cell::Cell::new(None);
    Host::until("the attach process", || {
        let out = std::process::Command::new("pgrep").args(["-P", &h.pid().to_string()]).output().unwrap();
        attach.set(String::from_utf8_lossy(&out.stdout).lines().next().and_then(|l| l.trim().parse::<i32>().ok()));
        attach.get().is_some()
    });

    // every transport is a child of attach (via the link), with an empty signal mask
    let pids = std::fs::read_to_string(r.fake.path().join("pids")).unwrap();
    for pid in pids.lines().map(str::trim) {
        let out = std::process::Command::new("ps").args(["-o", "sigmask=,command=", "-p", pid]).output().unwrap();
        let line = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if line.contains("server proxy") {
            assert!(line.starts_with("0 "), "ssh inherited a blocked signal: {line}");
        }
    }

    // SAFETY: a signal to the attach process this test started
    unsafe { libc::kill(attach.get().unwrap(), signal) };
    assert_restored(&h, 128 + signal);
}

#[test]
fn sigterm_restores_the_host_over_ssh() {
    killed_by(libc::SIGTERM);
}

#[test]
fn sighup_restores_the_host_over_ssh() {
    killed_by(libc::SIGHUP);
}
