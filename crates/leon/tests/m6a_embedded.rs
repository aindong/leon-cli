//! M6a acceptance (.agent/plans/2026-11-14-m6a-app.md): an in-process server answers the real
//! CLI, `session.focus_window`, and the `leon app` entry point. (The socket takeover rule went with
//! M10: the app attaches to whatever server owns the socket, tests/m10_attach.rs.)

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::{Duration, Instant};

use serde_json::json;

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
