//! M6a acceptance (.agent/plans/2026-11-14-m6a-app.md): the app's embedded server answers the
//! real CLI, the socket takeover rule, `session.focus_window`, and the `leon app` entry point.

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

struct Env {
    dir: tempfile::TempDir,
}

impl Env {
    fn new() -> Self {
        Env { dir: tempfile::Builder::new().permissions(std::fs::Permissions::from_mode(0o700)).tempdir().unwrap() }
    }

    fn cmd(&self) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_leon"));
        c.env("LEON_RUNTIME_DIR", self.dir.path())
            .env("LEON_STATE_DIR", self.dir.path().join("state"))
            .env("SHELL", "/bin/sh")
            .env_remove("LEON_SESSION")
            .env_remove("LEON_BLOCK")
            .env_remove("LEON_AUTOSTART")
            .env_remove("LEON_IDLE_EXIT_SECS");
        c
    }

    fn run(&self, args: &[&str]) -> Output {
        self.cmd().args(args).output().unwrap()
    }

    fn ok(&self, args: &[&str]) -> String {
        let out = self.run(args);
        assert!(out.status.success(), "leon {args:?} failed: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8(out.stdout).unwrap()
    }

    fn socket(&self) -> PathBuf {
        self.dir.path().join("server.sock")
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = self.cmd().args(["--autostart=false", "server", "stop"]).output();
    }
}

fn wait_for(what: &str, mut f: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(10);
    while Instant::now() < end {
        if f() {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("timed out waiting for {what}");
}

#[test]
fn the_embedded_server_answers_the_cli() {
    let env = Env::new();
    let app = leon_app::embedded::start_at(env.socket()).expect("embedded server starts");
    let created = app.call("session.create", json!({ "label": "from-app" })).unwrap();
    let sid = created["session_id"].as_str().unwrap().to_string();

    let ls = env.ok(&["--autostart=false", "ls"]);
    assert!(ls.contains("from-app") && ls.contains(&sid), "leon ls: {ls}");

    env.ok(&["--autostart=false", "-s", &sid, "run", "--", "sleep", "30"]);
    let windows = app.ctx().state().session(&sid).unwrap().windows.len();
    assert_eq!(windows, 2, "the CLI's window is in the app's in-process state");

    assert!(!app.ctx().is_autostarted(), "the app's server never idle-exits");
    app.stop();
    assert!(std::fs::symlink_metadata(env.socket()).is_err(), "stopping removes the socket");
    assert_ne!(env.run(&["--autostart=false", "ls"]).status.code(), Some(0));
}

#[test]
fn leon_server_stop_ends_the_embedded_server() {
    let env = Env::new();
    let app = leon_app::embedded::start_at(env.socket()).expect("embedded server starts");
    assert!(!app.stopped());
    env.ok(&["--autostart=false", "server", "stop"]);
    wait_for("the embedded server to stop", || app.stopped());
}

#[test]
fn a_busy_server_keeps_its_socket() {
    let env = Env::new();
    let sid = env.ok(&["new", "busy"]).trim().to_string(); // autostarts a server with a session
    let err = leon_app::embedded::start_at(env.socket()).err().expect("start refuses");
    let socket = env.socket().display().to_string();
    assert!(err.contains(&socket) && err.contains("leon server stop"), "{err}");
    let status: Value = serde_json::from_str(&env.ok(&["--autostart=false", "--json", "server", "status"])).unwrap();
    assert!(err.contains(&status["pid"].to_string()), "names the pid: {err}");
    assert!(env.ok(&["--autostart=false", "ls"]).contains(&sid), "the other server still runs");

    // `leon` with no subcommand (or only global flags) takes the app path and refuses the same way.
    for args in [&[][..], &["-s", "x"][..]] {
        let out = env.run(args);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.status.code(), Some(1), "leon {args:?}: {stderr}");
        assert!(stderr.contains("leon server stop") && stderr.contains(&socket), "leon {args:?}: {stderr}");
    }
}

#[test]
fn an_idle_server_is_replaced() {
    let env = Env::new();
    env.ok(&["server", "start"]);
    wait_for("the server", || env.run(&["--autostart=false", "server", "status"]).status.success());
    let old: Value = serde_json::from_str(&env.ok(&["--autostart=false", "--json", "server", "status"])).unwrap();
    let old_pid = old["pid"].as_i64().unwrap() as i32;
    let started = Instant::now();
    let app = leon_app::embedded::start_at(env.socket()).expect("takes over from an idle server");
    assert!(started.elapsed() < Duration::from_secs(10));
    let status: Value = serde_json::from_str(&env.ok(&["--autostart=false", "--json", "server", "status"])).unwrap();
    assert_eq!(status["pid"], json!(std::process::id()), "the app's server answers now");
    // SAFETY: signal 0 only checks whether the pid exists.
    let gone = unsafe { libc::kill(old_pid, 0) } != 0;
    assert!(gone, "the old server exited");
    app.stop();
}

#[test]
fn focus_window_makes_a_tab_active() {
    let env = Env::new();
    let sid = env.ok(&["new"]).trim().to_string();
    let first = env.ok(&["-s", &sid, "window", "ls"]);
    let w1 = first.split_whitespace().last().unwrap().to_string();
    env.ok(&["-s", &sid, "window", "new"]);
    let params = json!({ "session_id": sid, "window_id": w1 }).to_string();
    env.ok(&["api", "call", "session.focus_window", &params]);
    let ls = env.ok(&["-s", &sid, "window", "ls"]);
    let active: Vec<&str> = ls.lines().filter(|l| l.contains('*')).collect();
    assert_eq!(active.len(), 1, "{ls}");
    assert!(active[0].contains(&w1), "{ls}");
}

#[test]
fn leon_app_is_a_command() {
    let env = Env::new();
    assert!(env.ok(&["--help"]).lines().any(|l| l.trim_start().starts_with("app ")), "leon --help lists app");
    assert!(env.ok(&["app", "--help"]).to_lowercase().contains("app"));
}
