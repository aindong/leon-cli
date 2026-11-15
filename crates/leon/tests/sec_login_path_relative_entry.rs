//! Security review proof: a relative PATH entry from the login profile must not make `leon run`
//! execute a program from the block's working directory (an untrusted checkout).

mod support;

use std::os::unix::fs::PermissionsExt;
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
fn a_relative_login_path_entry_does_not_run_the_checkouts_program() {
    let env = Env::new();
    let home = env.dir.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(home.join(".profile"), "PATH=\"bin:$PATH\"; export PATH\n").unwrap();
    let repo = env.dir.path().join("repo");
    std::fs::create_dir_all(repo.join("bin")).unwrap();
    let planted = repo.join("bin/dd-planted");
    std::fs::write(&planted, "#!/bin/sh\nexit 42\n").unwrap();
    std::fs::set_permissions(&planted, std::fs::Permissions::from_mode(0o755)).unwrap();

    let started = launchd(&env, &home).args(["server", "start"]).output().unwrap();
    assert!(started.status.success(), "{}", String::from_utf8_lossy(&started.stderr));
    let out = launchd(&env, &home)
        .args(["run", "--wait", "--cwd", repo.to_str().unwrap(), "--", "dd-planted"])
        .output()
        .unwrap();
    let _ = launchd(&env, &home).args(["server", "stop", "--discard"]).output();
    assert_ne!(out.status.code(), Some(42), "a relative PATH entry from the profile ran the checkout's bin/dd-planted");
}
