//! Daily-driver criterion 1 (.agent/plans/2026-11-15-daily-driver-fixes.md): a server started the
//! way Leon.app starts it from Finder (launchd's bare PATH, no shell around it) runs programs with
//! the login shell's PATH, as docs/spec/cli.md says ("Commands run with the login shell's env").

mod support;

use std::os::unix::fs::PermissionsExt;
use std::process::Command;

use support::Env;

/// launchd gives apps `/usr/bin:/bin:/usr/sbin:/sbin`; the test uses the part every login shell
/// keeps (Debian's /etc/profile drops the sbin dirs for a non-root user).
const LAUNCHD_PATH: &str = "/usr/bin:/bin";

/// `leon` with only what launchd gives an app: HOME, SHELL and the bare PATH.
fn launchd(env: &Env, home: &std::path::Path) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_leon"));
    c.env_clear()
        .env("HOME", home)
        .env("SHELL", "/bin/sh")
        .env("PATH", LAUNCHD_PATH)
        .env("LEON_RUNTIME_DIR", env.dir.path())
        .env("LEON_STATE_DIR", env.state_dir())
        .env("LEON_CONFIG", env.dir.path().join("no-init.lua"));
    c
}

#[test]
fn a_server_started_without_a_shell_runs_programs_on_the_login_shells_path() {
    let env = Env::new();
    let home = env.dir.path().join("home");
    let tools = home.join("tools");
    std::fs::create_dir_all(&tools).unwrap();
    // what a user's ~/.profile (or ~/.zprofile) does: put their tools on PATH for login shells
    std::fs::write(home.join(".profile"), "PATH=\"$HOME/tools:$PATH\"; export PATH\n").unwrap();
    for (name, body) in [("dd-hello-tool", "exit 0"), ("uname", "echo tools-uname")] {
        let tool = tools.join(name);
        std::fs::write(&tool, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&tool, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    let started = launchd(&env, &home).args(["server", "start"]).output().unwrap();
    assert!(started.status.success(), "{}", String::from_utf8_lossy(&started.stderr));
    let out = launchd(&env, &home).args(["run", "--wait", "--", "dd-hello-tool"]).output().unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "the tool on the login shell's PATH runs: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    // in the login shell's order, as in the user's terminal: their `uname` before /usr/bin's
    let b = launchd(&env, &home).args(["run", "--keep-open", "--", "uname"]).output().unwrap();
    let b = String::from_utf8(b.stdout).unwrap();
    env.screen_until(b.trim(), "the tools uname", |s| {
        s.contains("tools-uname") || s.contains("Darwin") || s.contains("Linux")
    });
    assert!(env.capture(b.trim()).contains("tools-uname"), "{}", env.capture(b.trim()));
}

#[test]
fn a_server_started_from_a_shell_keeps_its_own_path_first() {
    let env = Env::new();
    let home = env.dir.path().join("home");
    let mine = env.dir.path().join("mine");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&mine).unwrap();
    // a dir only this terminal has (a venv, an nvm node), ahead of everything
    let path = format!("{}:{LAUNCHD_PATH}", mine.display());
    let leon = |args: &[&str]| launchd(&env, &home).env("PATH", &path).args(args).output().unwrap();
    assert!(leon(&["server", "start"]).status.success());
    let b = String::from_utf8(leon(&["run", "--keep-open", "--", "sh", "-c", "echo \"P=$PATH\""]).stdout).unwrap();
    let b = b.trim();
    let end = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let line = loop {
        let text = String::from_utf8(leon(&["capture", "-b", b]).stdout).unwrap();
        if let Some(l) = text.lines().find(|l| l.starts_with("P=")) {
            break l.to_string();
        }
        assert!(std::time::Instant::now() < end, "no PATH line in:\n{text}");
        std::thread::sleep(std::time::Duration::from_millis(50));
    };
    let dirs: Vec<&str> = line.trim_start_matches("P=").split(':').collect();
    // LEON_BIN_DIR comes first, then this terminal's own dir
    assert_eq!(dirs.get(1).copied(), Some(mine.to_str().unwrap()), "{line}");
}
