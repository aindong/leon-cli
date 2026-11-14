//! M1 acceptance tests (.agent/plans/2026-11-14-m1-server-sessions.md): the `leon` binary,
//! black box. Each test gets its own runtime dir, so its own server, and stops it on drop.

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;

struct Env {
    dir: tempfile::TempDir,
}

impl Env {
    fn new() -> Self {
        Env { dir: tempfile::tempdir().unwrap() }
    }

    fn cmd(&self) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_leon"));
        c.env("LEON_RUNTIME_DIR", self.dir.path())
            .env_remove("LEON_SESSION")
            .env_remove("LEON_AUTOSTART")
            .env_remove("LEON_IDLE_EXIT_SECS");
        c
    }

    fn run(&self, args: &[&str]) -> Output {
        self.cmd().args(args).output().unwrap()
    }

    fn ok(&self, args: &[&str]) -> String {
        let out = self.run(args);
        assert!(
            out.status.success(),
            "leon {args:?} failed ({:?}): {}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap()
    }

    fn json(&self, args: &[&str]) -> Value {
        let out = self.ok(args);
        serde_json::from_str(&out).unwrap_or_else(|e| panic!("leon {args:?}: not JSON ({e}): {out}"))
    }

    fn code(&self, args: &[&str]) -> i32 {
        self.run(args).status.code().unwrap()
    }

    fn socket(&self) -> PathBuf {
        self.dir.path().join("server.sock")
    }

    /// Poll until `f` is true or the timeout passes.
    fn wait_for(&self, what: &str, secs: u64, mut f: impl FnMut() -> bool) {
        let end = Instant::now() + Duration::from_secs(secs);
        while Instant::now() < end {
            if f() {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        panic!("timed out waiting for {what}");
    }

    fn alive(&self) -> bool {
        self.code(&["--autostart=false", "server", "status"]) == 0
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = self.cmd().args(["--autostart=false", "server", "stop"]).output();
    }
}

fn spawn_foreground(env: &Env) -> Child {
    let child = env
        .cmd()
        .args(["server", "run"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    env.wait_for("server socket", 10, || env.alive());
    child
}

#[test]
fn c1_server_run_status_stop() {
    let env = Env::new();
    let mut child = spawn_foreground(&env);
    let st = env.json(&["server", "status", "--json"]);
    assert_eq!(st["state"], "running");
    assert_eq!(st["pid"].as_u64().unwrap(), u64::from(child.id()));
    assert!(st["version"].as_str().is_some_and(|v| !v.is_empty()));
    assert_eq!(st["socket"].as_str().unwrap(), env.socket().to_str().unwrap());
    let mode = std::fs::metadata(env.socket()).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600, "socket mode");
    env.ok(&["server", "stop"]);
    env.wait_for("foreground server to exit", 10, || child.try_wait().unwrap().is_some());
    assert_eq!(env.code(&["--autostart=false", "server", "status"]), 4);
}

#[test]
fn c2_server_start_is_detached_and_idempotent() {
    let env = Env::new();
    // `output()` waits for the command (and anything holding its stdout) to finish, so returning proves it detached.
    let first = env.run(&["server", "start"]);
    assert!(first.status.success(), "{}", String::from_utf8_lossy(&first.stderr));
    // --autostart=false: status must find the server `start` left, not start its own.
    let pid1 = env.json(&["--autostart=false", "server", "status", "--json"])["pid"].clone();
    assert!(pid1.is_u64());
    env.ok(&["server", "start"]);
    let pid2 = env.json(&["--autostart=false", "server", "status", "--json"])["pid"].clone();
    assert_eq!(pid1, pid2);
}

#[test]
fn c3_autostart() {
    let env = Env::new();
    assert_eq!(env.code(&["--autostart=false", "ls"]), 4);
    let out = env.cmd().env("LEON_AUTOSTART", "0").arg("ls").output().unwrap();
    assert_eq!(out.status.code(), Some(4));
    assert!(!env.alive(), "nothing should have started");
    env.ok(&["ls"]);
    assert!(env.alive(), "ls should autostart a server");
}

#[test]
fn c4_sessions() {
    let env = Env::new();
    let web = env.ok(&["new", "web"]).trim().to_string();
    assert!(web.starts_with("session:"), "new prints the id: {web}");
    let api = env.ok(&["session", "new", "api"]).trim().to_string();
    assert!(api.starts_with("session:"));
    let ls = env.json(&["ls", "--json"]);
    let ls = ls.as_array().unwrap();
    assert_eq!(ls.len(), 2);
    assert_eq!(ls[0]["session_id"], web.as_str());
    assert_eq!(ls[0]["label"], "web");
    assert_eq!(ls[0]["windows"], 1);
    assert_eq!(ls[1]["label"], "api");
    assert_eq!(ls[1]["session_id"], api.as_str());
    assert_eq!(ls[1]["windows"], 1);
    let ins = env.json(&["session", "inspect", "web", "--json"]);
    assert_eq!(ins["session_id"], web.as_str());
    assert_eq!(ins["windows"].as_array().unwrap().len(), 1);
    env.ok(&["session", "rename", "web", "front"]);
    assert_eq!(env.json(&["ls", "--json"])[0]["label"], "front");
    env.ok(&["session", "kill", "front"]);
    env.ok(&["kill", "api"]);
    assert_eq!(env.json(&["ls", "--json"]).as_array().unwrap().len(), 0);
}

#[test]
fn c5_windows() {
    let env = Env::new();
    env.ok(&["new", "web"]);
    let w = env.ok(&["window", "new", "-s", "web", "logs"]).trim().to_string();
    assert!(w.starts_with("window:"), "window new prints the id: {w}");
    let ls = env.json(&["window", "ls", "-s", "web", "--json"]);
    let ls = ls.as_array().unwrap();
    assert_eq!(ls.len(), 2);
    assert_eq!(ls[1]["window_id"], w.as_str());
    assert_eq!(ls[1]["label"], "logs");
    assert_eq!(ls[1]["position"], 2);
    assert!(ls.iter().filter(|x| x["active"] == true).count() == 1, "exactly one active window");
    env.ok(&["window", "rename", "-s", "web", "logs", "build"]);
    assert_eq!(env.json(&["window", "ls", "-s", "web", "--json"])[1]["label"], "build");
    env.ok(&["window", "close", "-s", "web", "build"]);
    let left = env.json(&["window", "ls", "-s", "web", "--json"]);
    assert_eq!(left.as_array().unwrap().len(), 1);
    assert_ne!(left[0]["window_id"], w.as_str(), "the closed window is the one that's gone");
}

#[test]
fn c6_target_resolution() {
    let env = Env::new();
    let a = env.ok(&["new", "Alpha"]).trim().to_string();
    let b = env.ok(&["new", "beta"]).trim().to_string();
    let id_of = |q: &str| env.json(&["session", "inspect", q, "--json"])["session_id"].as_str().unwrap().to_string();
    assert_eq!(id_of(&a), a, "full id");
    assert_eq!(id_of("alpha"), a, "case-insensitive label");
    assert_eq!(id_of("2"), b, "1-based position");
    assert_eq!(id_of(&b[b.len() - 6..]), b, "id suffix");
    assert_eq!(env.code(&["session", "inspect", "nope"]), 3, "unknown target");
    assert_eq!(env.code(&["session", "inspect", &b[b.len() - 3..]]), 3, "suffix shorter than 4 is not a match");
    // Order: a label beats a position, and a label beats an ID suffix.
    let one = env.ok(&["new", "1"]).trim().to_string(); // at position 3, labelled "1"
    assert_eq!(id_of("1"), one, "label before position");
    let suffix = a[a.len() - 5..].to_string();
    let named = env.ok(&["new", &suffix]).trim().to_string();
    assert_eq!(id_of(&suffix), named, "label before ID suffix");
    let b2 = env.ok(&["new", "beta"]).trim().to_string();
    let out = env.run(&["session", "inspect", "beta"]);
    assert_eq!(out.status.code(), Some(3), "ambiguous label");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains(&b) && err.contains(&b2), "ambiguity lists both matches: {err}");
}

#[test]
fn c7_bad_command_line_exits_2() {
    let env = Env::new();
    assert_eq!(env.code(&["no-such-command"]), 2);
    assert_eq!(env.code(&["ls", "--no-such-flag"]), 2);
}

#[test]
fn c8_api() {
    let env = Env::new();
    let list = env.json(&["api", "list", "--json"]);
    let list = list.as_array().unwrap();
    let names: Vec<&str> = list.iter().map(|m| m["name"].as_str().unwrap()).collect();
    for m in ["help.all", "session.create", "session.list", "server.status", "target.resolve"] {
        assert!(names.contains(&m), "api list has {m}: {names:?}");
    }
    assert!(list.iter().all(|m| m["description"].as_str().is_some_and(|d| !d.is_empty())));
    assert!(env.ok(&["api", "list"]).contains("session.create"));
    let d = env.json(&["api", "describe", "session.create", "--json"]);
    assert_eq!(d["input_schema"]["type"], "object");
    assert!(d["output_schema"].is_object());
    let r = env.json(&["api", "call", "session.create", r#"{"label":"x"}"#]);
    let sid = r["session_id"].as_str().unwrap();
    assert!(env.ok(&["ls"]).contains("x"));
    assert!(env.json(&["ls", "--json"]).as_array().unwrap().iter().any(|s| s["session_id"] == sid));
    let out = env.run(&["api", "call", "no.such.method"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("method_not_found"));
}

#[test]
fn c9_autostarted_server_exits_when_idle() {
    let env = Env::new();
    let out = env.cmd().env("LEON_IDLE_EXIT_SECS", "1").arg("ls").output().unwrap();
    assert!(out.status.success());
    // Each status check is a connection and resets the idle clock, so check slowly.
    let mut gone = false;
    for _ in 0..5 {
        std::thread::sleep(Duration::from_millis(2500));
        if !env.alive() {
            gone = true;
            break;
        }
    }
    assert!(gone, "idle autostarted server should exit");

    // A server from `server run` stays up past its idle period.
    let mut child = env.cmd().env("LEON_IDLE_EXIT_SECS", "1").args(["server", "run"])
        .stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();
    env.wait_for("server socket", 10, || env.alive());
    std::thread::sleep(Duration::from_secs(3));
    assert!(child.try_wait().unwrap().is_none(), "server run must not idle-exit");
    env.ok(&["server", "stop"]);
    env.wait_for("foreground server to exit", 10, || child.try_wait().unwrap().is_some());

    // A server from `server start` stays up past its idle period.
    let started = env.cmd().env("LEON_IDLE_EXIT_SECS", "1").args(["server", "start"]).output().unwrap();
    assert!(started.status.success());
    std::thread::sleep(Duration::from_secs(3));
    assert!(env.alive(), "server start must not idle-exit");
}
