//! M17 criterion 10: the host terminal comes back cooked when `leon attach` is killed (SIGTERM,
//! SIGHUP, SIGKILL) or its server goes away, and killing the client leaves the server and its programs.

mod support;

use support::host::{assert_restored, attach_then_stty, Host};
use support::Env;

/// The `leon attach` process under the host's shell.
fn attach_pid(h: &Host) -> i32 {
    let pid = std::cell::Cell::new(None);
    Host::until("the attach process", || {
        let out = std::process::Command::new("pgrep").args(["-P", &h.pid().to_string()]).output().unwrap();
        pid.set(String::from_utf8_lossy(&out.stdout).lines().next().and_then(|l| l.trim().parse().ok()));
        pid.get().is_some()
    });
    pid.get().unwrap()
}

fn killed_by(signal: i32, rc: i32) {
    let env = Env::new();
    let block = env.ok(&["run", "--", "sh"]);
    let session = env.json(&["ls", "--json"])[0]["session_id"].as_str().unwrap().to_string();
    // SIGKILL can't be caught: a watchdog process restores the host after the client dies, so the
    // script gives it a moment (as a user typing their next command does) before checking
    let script = if signal == libc::SIGKILL {
        let wait = "i=0; while stty -a | grep -q -- -icanon && [ $i -lt 100 ]; do sleep 0.05; i=$((i+1)); done";
        attach_then_stty(&session).replacen("; f=$(", &format!("; {wait}; f=$("), 1)
    } else {
        attach_then_stty(&session)
    };
    let h = Host::spawn(&env, &script, &[]);
    h.wait_for("attached", |_| h.alt_screen() && h.last_row().contains("leon"));
    // SAFETY: a signal to the attach process this test started
    unsafe { libc::kill(attach_pid(&h), signal) };
    assert_restored(&h, rc);
    let b = env.json(&["block", "inspect", &block, "--json"]);
    assert!(b["exit_code"].is_null(), "killing the client must leave the program running: {b}");
}

#[test]
fn sigterm_restores_the_host() {
    killed_by(libc::SIGTERM, 128 + libc::SIGTERM);
}

#[test]
fn sighup_restores_the_host() {
    killed_by(libc::SIGHUP, 128 + libc::SIGHUP);
}

#[test]
fn sigkill_restores_the_host() {
    killed_by(libc::SIGKILL, 128 + libc::SIGKILL);
}

#[test]
fn the_server_stopping_restores_the_host() {
    let env = Env::new();
    env.ok(&["run", "--", "sh"]);
    let session = env.json(&["ls", "--json"])[0]["session_id"].as_str().unwrap().to_string();
    let h = Host::spawn(&env, &attach_then_stty(&session), &[]);
    h.wait_for("attached", |_| h.alt_screen() && h.last_row().contains("leon"));
    env.ok(&["server", "stop"]);
    assert_restored(&h, 1);
    assert!(h.screen().contains("server"), "it says the server went away:\n{}", h.screen());
}
