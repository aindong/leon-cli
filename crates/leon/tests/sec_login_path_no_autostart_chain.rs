//! Security review proof: a profile or rc line that runs leon must not make the server's PATH
//! probe autostart servers whose own probes run it again.

mod support;

use std::process::Command;

use support::Env;

fn launchd(env: &Env, home: &std::path::Path) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_leon"));
    c.env_clear()
        .env("HOME", home)
        .env("SHELL", "/bin/sh")
        .env("PATH", "/usr/bin:/bin")
        .env("LEON_RUNTIME_DIR", env.dir.path())
        .env("LEON_STATE_DIR", env.state_dir())
        .env("LEON_CONFIG", env.dir.path().join("no-init.lua"));
    c
}

#[test]
fn a_profile_that_runs_leon_does_not_chain_server_starts() {
    let env = Env::new();
    let home = env.dir.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let leon = env!("CARGO_BIN_EXE_leon");
    // each probe that runs the profile appends one byte; `leon ls` autostarts when no socket is up
    std::fs::write(home.join(".profile"), format!("printf x >> \"$HOME/runs\"\n'{leon}' ls >/dev/null 2>&1\n"))
        .unwrap();
    let started = launchd(&env, &home).args(["server", "start"]).output().unwrap();
    let runs = std::fs::read(home.join("runs")).map(|b| b.len()).unwrap_or(0);
    let _ = launchd(&env, &home).args(["server", "stop", "--discard"]).output();
    assert!(runs <= 1, "the login-shell probe autostarted {runs} nested servers' probes");
    assert!(started.status.success(), "{}", String::from_utf8_lossy(&started.stderr));
}
