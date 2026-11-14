//! M2 acceptance tests (.agent/plans/2026-11-14-m2-terminal-blocks.md): terminal blocks through the
//! real `leon` binary. Each test gets its own private runtime dir and server; `SHELL=/bin/sh` makes
//! the login shell predictable. Screens are polled with a timeout, never read after a fixed sleep.

use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;

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
            .env("LEON_STATE_DIR", std::path::Path::new(self.dir.path()).join("state"))
            .env("SHELL", "/bin/sh")
            .env_remove("LEON_SESSION")
            .env_remove("LEON_BLOCK")
            .env_remove("LEON_SERVER")
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
        String::from_utf8(out.stdout).unwrap().trim().to_string()
    }

    fn json(&self, args: &[&str]) -> Value {
        let out = self.ok(args);
        serde_json::from_str(&out).unwrap_or_else(|e| panic!("leon {args:?}: not JSON ({e}): {out}"))
    }

    fn code(&self, args: &[&str]) -> i32 {
        self.run(args).status.code().unwrap()
    }

    fn socket(&self) -> String {
        self.dir.path().join("server.sock").display().to_string()
    }

    fn blocks(&self, session: &str) -> Vec<Value> {
        self.json(&["-s", session, "block", "ls", "--json"]).as_array().unwrap().clone()
    }

    fn capture(&self, block: &str) -> String {
        self.ok(&["capture", "-b", block])
    }

    /// Poll the block's screen until `pred` holds; returns that screen.
    fn screen_until(&self, block: &str, what: &str, pred: impl Fn(&str) -> bool) -> String {
        let mut last = String::new();
        let end = Instant::now() + Duration::from_secs(10);
        while Instant::now() < end {
            last = self.capture(block);
            if pred(&last) {
                return last;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        panic!("timed out waiting for {what}; screen was:\n{last}");
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = self.cmd().args(["--autostart=false", "server", "stop"]).output();
    }
}

fn wait_for(what: &str, secs: u64, mut f: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < end {
        if f() {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("timed out waiting for {what}");
}

fn has_line(screen: &str, line: &str) -> bool {
    screen.lines().any(|l| l.trim() == line)
}

fn alive(pid: i32) -> bool {
    // SAFETY: signal 0 only checks that the process exists.
    unsafe { libc::kill(pid, 0) == 0 }
}

/// A session with its own shell window, so closing a `run` window never destroys it.
fn session(env: &Env) -> String {
    env.ok(&["new"])
}

#[test]
fn c1_new_windows_get_a_login_shell_block() {
    let env = Env::new();
    let s = session(&env);
    let blocks = env.blocks(&s);
    assert_eq!(blocks.len(), 1, "a new session's window holds one block: {blocks:?}");
    let b = blocks[0]["block_id"].as_str().unwrap().to_string();
    assert!(b.starts_with("block:"), "{b}");
    assert_eq!(blocks[0]["command"][0], "/bin/sh", "the login shell from $SHELL: {blocks:?}");
    let view = env.json(&["-s", &s, "session", "inspect", "--json"]);
    assert_eq!(blocks[0]["window_id"], view["windows"][0]["window_id"]);
    // A login shell's $0 starts with '-'.
    env.ok(&["send", "-b", &b, "echo $0"]);
    env.ok(&["send-key", "-b", &b, "enter"]);
    env.screen_until(&b, "a login shell's $0", |s| has_line(s, "-sh"));

    env.ok(&["-s", &s, "window", "new"]);
    let blocks = env.blocks(&s);
    assert_eq!(blocks.len(), 2, "window new adds one shell block: {blocks:?}");
    assert_ne!(blocks[0]["window_id"], blocks[1]["window_id"]);
    assert_eq!(blocks[1]["command"][0], "/bin/sh");
}

#[test]
fn c2_wait_returns_the_exit_status() {
    let env = Env::new();
    let s = session(&env);
    let b = env.ok(&["-s", &s, "run", "--", "sh", "-c", "echo hi; exit 3"]);
    assert!(b.starts_with("block:"), "run prints the block id: {b}");
    // The block closes once its program exits; wait still knows the status afterwards.
    wait_for("the finished block to close", 10, || env.blocks(&s).iter().all(|x| x["block_id"] != b.as_str()));
    let out = env.run(&["wait", &b]);
    assert_eq!(out.status.code(), Some(3));
    assert!(
        out.stderr.is_empty(),
        "3 must be the child's status, not a target error: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // A wait that is already pending when the child exits.
    let b = env.ok(&["-s", &s, "run", "--", "sh", "-c", "sleep 1; exit 7"]);
    let t = Instant::now();
    assert_eq!(env.code(&["wait", &b]), 7);
    assert!(t.elapsed() >= Duration::from_millis(500), "wait returned before the child exited");
    wait_for("the finished block to close", 10, || env.blocks(&s).iter().all(|x| x["block_id"] != b.as_str()));
    assert_eq!(env.code(&["wait", &b]), 7);

    assert_eq!(env.code(&["-s", &s, "run", "--wait", "--", "sh", "-c", "exit 5"]), 5);
}

#[test]
fn c3_send_types_and_send_key_presses_enter() {
    let env = Env::new();
    let s = session(&env);
    let b = env.blocks(&s)[0]["block_id"].as_str().unwrap().to_string();
    env.ok(&["send", "-b", &b, "echo $((6*7))"]);
    env.ok(&["send-key", "-b", &b, "enter"]);
    env.screen_until(&b, "the command's output", |s| has_line(s, "42"));
    // With an implicit Enter these would be two commands printing A and B.
    env.ok(&["send", "-b", &b, "echo A"]);
    env.ok(&["send", "-b", &b, "echo B"]);
    env.ok(&["send-key", "-b", &b, "enter"]);
    let screen = env.screen_until(&b, "one joined command", |s| has_line(s, "Aecho B"));
    assert!(!has_line(&screen, "A"), "send pressed Enter:\n{screen}");

    let mut child = env.cmd().args(["send", "-b", &b]).stdin(Stdio::piped()).spawn().unwrap();
    use std::io::Write;
    child.stdin.take().unwrap().write_all(b"echo $((7*8))\n").unwrap();
    assert!(child.wait().unwrap().success());
    env.screen_until(&b, "output of a command sent on stdin", |s| has_line(s, "56"));
}

#[test]
fn c4_child_env_and_keep_open() {
    let env = Env::new();
    let s = session(&env);
    let script = r#"echo "S=$LEON_SESSION"; echo "B=$LEON_BLOCK"; echo "V=$LEON_SERVER"; echo "T=$TERM""#;
    let b = env.ok(&["-s", &s, "run", "--keep-open", "--", "sh", "-c", script]);
    let screen = env.screen_until(&b, "the env lines", |s| s.contains("T="));
    let trimmed = env.ok(&["capture", "--trim", "-b", &b]);
    for want in [format!("S={s}"), format!("B={b}"), format!("V={}", env.socket()), "T=xterm-256color".into()] {
        assert!(has_line(&screen, &want), "missing {want:?} in:\n{screen}");
        assert!(has_line(&trimmed, &want), "missing {want:?} in --trim capture:\n{trimmed}");
    }
    assert!(!trimmed.ends_with('\n') && !trimmed.lines().any(|l| l.ends_with(' ')), "--trim:\n{trimmed:?}");
    assert_eq!(env.code(&["wait", &b]), 0);
    assert!(env.blocks(&s).iter().any(|x| x["block_id"] == b.as_str()), "--keep-open keeps the block after exit");
}

#[test]
fn c5_a_finished_program_closes_its_block_and_window() {
    let env = Env::new();
    let s = session(&env);
    let b = env.ok(&["-s", &s, "run", "--", "true"]);
    wait_for("the block to close", 10, || env.blocks(&s).iter().all(|x| x["block_id"] != b.as_str()));
    let view = env.json(&["-s", &s, "session", "inspect", "--json"]);
    assert_eq!(view["windows"].as_array().unwrap().len(), 1, "the run window closed with its only block");
}

#[test]
fn c6_killing_a_session_or_window_ends_its_processes() {
    let env = Env::new();
    let s = session(&env);
    let b = env.ok(&["-s", &s, "run", "--", "sh", "-c", "sleep 300 & echo bg=$!; wait"]);
    let screen = env.screen_until(&b, "the background pid", |s| s.contains("bg="));
    let bg: i32 = screen.lines().find_map(|l| l.trim().strip_prefix("bg=")).unwrap().trim().parse().unwrap();
    let pid = env.json(&["block", "inspect", &b, "--json"])["pid"].as_i64().unwrap() as i32;
    assert!(alive(pid) && alive(bg));
    env.ok(&["kill", &s]);
    wait_for("the block's process to end", 5, || !alive(pid));
    wait_for("the background job to end", 5, || !alive(bg));

    let s = session(&env);
    env.ok(&["-s", &s, "window", "new"]);
    let shell = env.blocks(&s)[1].clone();
    let pid = shell["pid"].as_i64().unwrap() as i32;
    assert!(alive(pid));
    env.ok(&["-s", &s, "window", "close", "2"]);
    wait_for("the closed window's shell to end", 5, || !alive(pid));
}

#[test]
fn c7_wait_for_times_out_with_exit_1() {
    let env = Env::new();
    let s = session(&env);
    let b = env.ok(&["-s", &s, "run", "--", "sleep", "30"]);
    let t = Instant::now();
    let out = env.run(&["wait", &b, "--for", "200ms"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(t.elapsed() >= Duration::from_millis(200), "returned before the timeout: {:?}", t.elapsed());
    assert!(t.elapsed() < Duration::from_secs(5), "took {:?}", t.elapsed());
    assert!(String::from_utf8_lossy(&out.stderr).contains("timeout"), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(env.blocks(&s).iter().any(|x| x["block_id"] == b.as_str()), "a timed-out wait leaves the block alone");
}

#[test]
fn c8_pending_waits_do_not_stall_the_server() {
    let env = Env::new();
    let s = session(&env);
    let b = env.ok(&["-s", &s, "run", "--", "sleep", "300"]);
    let mut waits: Vec<_> =
        (0..40).map(|_| env.cmd().args(["wait", &b]).stdout(Stdio::null()).spawn().unwrap()).collect();
    for _ in 0..5 {
        let t = Instant::now();
        env.ok(&["ls"]);
        assert!(t.elapsed() < Duration::from_secs(5), "ls took {:?} with waits pending", t.elapsed());
    }
    assert!(waits.iter_mut().all(|w| w.try_wait().unwrap().is_none()), "waits should still be pending");
    env.ok(&["block", "close", &b]);
    for w in &mut waits {
        w.wait().unwrap(); // closing the block ends every wait
    }
}

#[test]
fn c9_send_key_binding_syntax() {
    let env = Env::new();
    let s = session(&env);
    let script = r#"trap 'echo got-int; exit 7' INT; echo ready; while :; do sleep 1; done"#;
    let b = env.ok(&["-s", &s, "run", "--", "sh", "-c", script]);
    env.screen_until(&b, "the script to be ready", |s| has_line(s, "ready"));
    env.ok(&["send-key", "-b", &b, "ctrl+c"]);
    assert_eq!(env.code(&["wait", &b, "--for", "10s"]), 7, "ctrl+c reached the program as SIGINT");

    let shell = env.blocks(&s)[0]["block_id"].as_str().unwrap().to_string();
    let out = env.run(&["send-key", "-b", &shell, "nosuchkey"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("nosuchkey"));
    // Several keys in one call, including arrows and escape: type "ab", go left, insert "X".
    env.ok(&["send", "-b", &shell, "echo ab"]);
    env.ok(&["send-key", "-b", &shell, "left", "escape", "tab"]);
    env.ok(&["send-key", "-b", &shell, "ctrl+u", "ctrl+c"]);
    env.ok(&["send", "-b", &shell, "echo after"]);
    env.ok(&["send-key", "-b", &shell, "enter"]);
    env.screen_until(&shell, "the shell to still answer", |s| has_line(s, "after"));
}

#[test]
fn c10_blocks_resolve_like_other_targets() {
    let env = Env::new();
    let s = session(&env);
    let b = env.ok(&["-s", &s, "run", "--label", "build", "--", "sleep", "300"]);
    assert_eq!(env.json(&["block", "inspect", "build", "--json"])["block_id"], b.as_str(), "by label");
    assert_eq!(env.json(&["block", "inspect", "BUILD", "--json"])["label"], "build", "case-insensitive");
    assert_eq!(env.json(&["block", "inspect", &b[b.len() - 8..], "--json"])["block_id"], b.as_str(), "by suffix");
    assert_eq!(env.json(&["-s", &s, "block", "inspect", "2", "--json"])["block_id"], b.as_str(), "by position");
    let info = env.json(&["block", "inspect", &b, "--json"]);
    assert_eq!(info["command"], serde_json::json!(["sleep", "300"]));
    assert!(info["window_id"].as_str().unwrap().starts_with("window:"));
    assert!(info["pid"].as_i64().unwrap() > 0);
    env.ok(&["capture", "-b", "build"]);
    assert_eq!(env.code(&["capture", "-b", "nope"]), 3, "an unknown block is a target error");

    let dup1 = env.ok(&["-s", &s, "run", "--label", "dup", "--", "sleep", "300"]);
    let dup2 = env.ok(&["-s", &s, "run", "--label", "dup", "--", "sleep", "300"]);
    let out = env.run(&["capture", "-b", "dup"]);
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(3), "an ambiguous block is a target error");
    assert!(err.contains(&dup1) && err.contains(&dup2), "ambiguity lists every match: {err}");

    // A full ID works without naming the session, even with several sessions.
    let _other = session(&env);
    env.ok(&["capture", "-b", &b]);

    let pid = info["pid"].as_i64().unwrap() as i32;
    env.ok(&["block", "close", "build"]);
    wait_for("the closed block's program to end", 5, || !alive(pid));
    assert!(env.blocks(&s).iter().all(|x| x["block_id"] != b.as_str()));
}

#[test]
fn c11_the_api_describes_blocks() {
    let env = Env::new();
    let s = session(&env);
    let help = env.json(&["api", "call", "help.all"]);
    let methods = help["methods"].as_array().unwrap();
    for name in
        ["block.close", "block.method", "block.event", "block.wait", "session.list_blocks", "session.describe_block"]
    {
        let m = methods.iter().find(|m| m["name"] == name).unwrap_or_else(|| panic!("help.all lacks {name}"));
        assert_eq!(m["input_schema"]["type"], "object", "{name}");
    }
    let types = help["block_types"].as_array().unwrap();
    let term = types.iter().find(|t| t["creator"] == "dev.leon.terminal").expect("a terminal block type");
    for name in ["write", "format", "size", "process"] {
        let full = format!("dev.leon.terminal.{name}");
        let m = term["methods"].as_array().unwrap().iter().find(|m| m["name"] == full.as_str());
        assert!(m.is_some_and(|m| m["input_schema"]["type"] == "object"), "block_types lacks {full}");
    }
    let b = env.blocks(&s)[0]["block_id"].as_str().unwrap().to_string();
    let params = serde_json::json!({ "session_id": s, "block_id": b, "method": "dev.leon.terminal.size" }).to_string();
    let size = env.json(&["api", "call", "block.method", &params]);
    assert_eq!((size["rows"].as_u64(), size["cols"].as_u64()), (Some(24), Some(80)), "{size}");
}

#[test]
fn c13_capture_includes_scrollback_and_run_takes_a_cwd() {
    let env = Env::new();
    let s = session(&env);
    let b = env.ok(&["-s", &s, "run", "--keep-open", "--", "seq", "1", "300"]);
    env.screen_until(&b, "all of seq's output", |s| has_line(s, "300"));
    assert!(has_line(&env.capture(&b), "1"), "the first line scrolled off but must be in the capture");

    let dir = tempfile::tempdir().unwrap();
    let real = dir.path().canonicalize().unwrap().display().to_string();
    let b = env.ok(&["-s", &s, "run", "--keep-open", "-c", dir.path().to_str().unwrap(), "--", "pwd", "-P"]);
    env.screen_until(&b, "pwd in the given dir", |s| has_line(s, &real));
}
