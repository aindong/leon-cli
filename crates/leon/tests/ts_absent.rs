//! Tailscale criterion 4 (.agent/plans/2026-11-16-tailscale-hosts.md): no Tailscale, or Tailscale
//! stopped, is one line, and everything else works as before.

mod support;

use std::process::Stdio;

use support::{tailscale::Fake, Env};

fn check(env: &Env, fake: Option<&Fake>, want: &str) {
    let mut c = env.cmd();
    if let Some(f) = fake {
        f.apply(&mut c);
    }
    let out = c.args(["hosts", "add", "--tailscale"]).stdin(Stdio::null()).output().unwrap();
    let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert_eq!(out.status.code(), Some(1), "{text}");
    assert_eq!(text.trim().lines().count(), 1, "one line: {text}");
    assert!(text.to_lowercase().contains(want), "{want:?}: {text}");

    // the rest works as before
    let mut c = env.cmd();
    if let Some(f) = fake {
        f.apply(&mut c);
    }
    assert!(c.args(["hosts", "add", "box", "ssh://box"]).status().unwrap().success());
    let mut c = env.cmd();
    if let Some(f) = fake {
        f.apply(&mut c);
    }
    let out = c.args(["hosts", "list"]).output().unwrap();
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "box  ssh://box", "as before");
    assert!(out.stderr.is_empty(), "{}", String::from_utf8_lossy(&out.stderr));
    env.ok(&["hosts", "remove", "box"]);
    if let Some(f) = fake {
        f.only_read();
    }
}

#[test]
fn not_installed() {
    let env = Env::new(); // its LEON_TAILSCALE names a file that isn't there
    check(&env, None, "isn't installed");
}

#[test]
fn stopped() {
    let env = Env::new();
    let fake = Fake::new("stopped.json");
    check(&env, Some(&fake), "stopped");
}

#[test]
fn not_running() {
    let env = Env::new();
    let fake = Fake::new("running.json");
    fake.set("status", "");
    fake.set("rc", "1");
    check(&env, Some(&fake), "isn't running");
}
