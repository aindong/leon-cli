//! Shared by the M3/M4 acceptance tests: a private runtime dir with its own server, and a reader
//! for `leon events` that never blocks the test (every read has a deadline).

#![allow(dead_code)]

use std::io::{BufRead, BufReader};
use std::os::unix::fs::PermissionsExt;
use std::process::{Child, Command, Output, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde_json::Value;

pub struct Env {
    pub dir: tempfile::TempDir,
}

impl Env {
    pub fn new() -> Self {
        Env { dir: tempfile::Builder::new().permissions(std::fs::Permissions::from_mode(0o700)).tempdir().unwrap() }
    }

    pub fn cmd(&self) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_leon"));
        c.env("LEON_RUNTIME_DIR", self.dir.path())
            .env("LEON_STATE_DIR", self.state_dir())
            // never the developer's own init.lua; tests that want a config set LEON_CONFIG again
            .env("LEON_CONFIG", self.dir.path().join("no-init.lua"))
            .env("SHELL", "/bin/sh")
            .env_remove("LEON_SESSION")
            .env_remove("LEON_BLOCK")
            .env_remove("LEON_SERVER")
            .env_remove("LEON_AUTOSTART")
            .env_remove("LEON_IDLE_EXIT_SECS");
        c
    }

    pub fn run(&self, args: &[&str]) -> Output {
        self.cmd().args(args).output().unwrap()
    }

    pub fn ok(&self, args: &[&str]) -> String {
        let out = self.run(args);
        assert!(
            out.status.success(),
            "leon {args:?} failed ({:?}): {}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap().trim().to_string()
    }

    pub fn json(&self, args: &[&str]) -> Value {
        let out = self.ok(args);
        serde_json::from_str(&out).unwrap_or_else(|e| panic!("leon {args:?}: not JSON ({e}): {out}"))
    }

    pub fn code(&self, args: &[&str]) -> i32 {
        self.run(args).status.code().unwrap()
    }

    /// Where this env's server saves its sessions (never the real ~/.local/state/leon).
    pub fn state_dir(&self) -> std::path::PathBuf {
        self.dir.path().join("state")
    }

    pub fn socket(&self) -> std::path::PathBuf {
        self.dir.path().join("server.sock")
    }

    /// A session whose first block runs `sleep 300`; returns (session id, block id).
    pub fn session(&self) -> (String, String) {
        let b = self.ok(&["run", "--", "sleep", "300"]);
        let v = self.json(&["block", "inspect", &b, "--json"]);
        let s = self.json(&["ls", "--json"])[0]["session_id"].as_str().unwrap().to_string();
        assert_eq!(v["block_id"], b.as_str());
        (s, b)
    }

    pub fn view(&self, session: &str) -> Value {
        self.json(&["-s", session, "session", "inspect", "--json"])
    }

    /// The first window's layout, focused block and zoomed block.
    pub fn window(&self, session: &str, i: usize) -> Value {
        self.view(session)["windows"][i].clone()
    }

    pub fn capture(&self, block: &str) -> String {
        self.ok(&["capture", "-b", block])
    }

    /// Poll the block's screen until `pred` holds; returns that screen.
    pub fn screen_until(&self, block: &str, what: &str, pred: impl Fn(&str) -> bool) -> String {
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

    /// Start `leon events ARGS --json` and wait for its snapshot line.
    pub fn events(&self, args: &[&str]) -> Stream {
        let mut child = self.cmd().arg("events").args(args).arg("--json").stdout(Stdio::piped()).spawn().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (tx, lines) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { return };
                if tx.send(line).is_err() {
                    return;
                }
            }
        });
        let mut s = Stream { child, lines, seq: 0 };
        let first = s.line(Duration::from_secs(10)).expect("leon events printed no snapshot");
        let snap: Value = serde_json::from_str(&first).unwrap_or_else(|e| panic!("snapshot not JSON ({e}): {first}"));
        assert!(snap.get("snapshot").is_some(), "the first line is the snapshot: {first}");
        s.seq = snap["seq"].as_u64().unwrap_or_else(|| panic!("snapshot has a seq: {first}"));
        s
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = self.cmd().args(["--autostart=false", "server", "stop"]).output();
    }
}

/// A running `leon events --json`. `next` checks that seq rises by exactly one.
pub struct Stream {
    pub child: Child,
    pub lines: mpsc::Receiver<String>,
    /// The last seq seen (the snapshot's at first).
    pub seq: u64,
}

impl Stream {
    pub fn line(&mut self, timeout: Duration) -> Option<String> {
        self.lines.recv_timeout(timeout).ok()
    }

    /// The next event, unwrapped: (type or block-event name, payload, the whole line).
    pub fn next(&mut self) -> Ev {
        let line = self.line(Duration::from_secs(10)).expect("timed out waiting for an event");
        let v: Value = serde_json::from_str(&line).unwrap_or_else(|e| panic!("event not JSON ({e}): {line}"));
        let seq = v["seq"].as_u64().unwrap_or_else(|| panic!("event without seq: {line}"));
        assert_eq!(seq, self.seq + 1, "seq must rise by exactly one: {line}");
        self.seq = seq;
        Ev::new(&v["event"])
    }

    /// Skip events until one named `name` arrives (each still checked for gaps).
    pub fn until(&mut self, name: &str) -> Ev {
        for _ in 0..200 {
            let e = self.next();
            if e.name == name {
                return e;
            }
        }
        panic!("no {name} event in 200 events");
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// One event, unwrapped from the spec's CLI JSON shape.
#[derive(Debug)]
pub struct Ev {
    /// `session_created`, `window_created`, or a block event's name (`pwd_changed`).
    pub name: String,
    pub session_id: Option<String>,
    pub block_id: Option<String>,
    pub payload: Value,
}

impl Ev {
    pub fn new(e: &Value) -> Ev {
        let s = |v: &Value| v.as_str().map(str::to_string);
        if e["type"] != "session_event" {
            return Ev {
                name: s(&e["type"]).unwrap_or_default(),
                session_id: s(&e["payload"]["session_id"]).or_else(|| s(&e["payload"]["session"]["session_id"])),
                block_id: None,
                payload: e["payload"].clone(),
            };
        }
        let inner = &e["payload"]["event"];
        let session_id = s(&e["payload"]["session_id"]);
        if inner["type"] == "block_event" {
            let p = &inner["payload"];
            assert_eq!(p["creator_name"], "dev.leon.terminal", "{e}");
            return Ev {
                name: s(&p["name"]).unwrap_or_default(),
                session_id,
                block_id: s(&p["block_id"]),
                payload: p["payload"].clone(),
            };
        }
        Ev {
            name: s(&inner["type"]).unwrap_or_default(),
            session_id,
            block_id: None,
            payload: inner["payload"].clone(),
        }
    }
}

/// `"A"`, or `"h(A,B)"` / `"v(A,B)"` with block ids replaced by names, for readable tree asserts.
pub fn shape(node: &Value, names: &[(&str, &str)]) -> String {
    if let Some(b) = node.get("block") {
        let id = b["block_id"].as_str().unwrap();
        return names.iter().find(|(i, _)| *i == id).map_or(id.to_string(), |(_, n)| n.to_string());
    }
    let s = &node["split"];
    let d = if s["direction"] == "horizontal" { "h" } else { "v" };
    format!("{d}({},{})", shape(&s["before"], names), shape(&s["after"], names))
}

pub fn revision(v: &Value) -> u64 {
    v["revision"].as_u64().unwrap_or_else(|| panic!("no revision in {v}"))
}
