//! Agent orchestration tests: a server whose panes get a private HOME, CLAUDE_CONFIG_DIR and
//! CODEX_HOME (never the real ~/.claude, ~/.codex or ~/.agents), fake agents written as `sh`
//! scripts, and leon run as if from inside a block.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, Instant};

use serde_json::Value;

use super::Env;

pub struct Orch {
    pub env: Env,
    pub home: PathBuf,
}

/// `c` with the private agent homes under `home`.
pub fn private(mut c: Command, home: &Path) -> Command {
    c.env("HOME", home).env("CLAUDE_CONFIG_DIR", home.join(".claude")).env("CODEX_HOME", home.join(".codex"));
    c
}

impl Orch {
    /// A started server: started here, so it and every pane it opens get the private homes.
    pub fn new() -> Orch {
        let env = Env::new();
        let home = env.dir.path().join("home");
        std::fs::create_dir(&home).unwrap();
        std::fs::set_permissions(&home, std::fs::Permissions::from_mode(0o700)).unwrap();
        let out = private(env.cmd(), &home).args(["server", "start"]).output().unwrap();
        assert!(out.status.success(), "server start: {}", String::from_utf8_lossy(&out.stderr));
        Orch { env, home }
    }

    pub fn cmd(&self) -> Command {
        private(self.env.cmd(), &self.home)
    }

    /// leon as a program inside block `block` of session `session` runs it.
    pub fn inside(&self, session: &str, block: &str) -> Command {
        let mut c = self.cmd();
        c.env("LEON_SESSION", session).env("LEON_BLOCK", block);
        c
    }

    pub fn run_inside(&self, session: &str, block: &str, args: &[&str]) -> Output {
        self.inside(session, block).args(args).output().unwrap()
    }

    pub fn ok_inside(&self, session: &str, block: &str, args: &[&str]) -> String {
        let out = self.run_inside(session, block, args);
        assert!(
            out.status.success(),
            "leon {args:?} in {block} failed ({:?}): {}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap().trim().to_string()
    }

    pub fn json_inside(&self, session: &str, block: &str, args: &[&str]) -> Value {
        let out = self.ok_inside(session, block, args);
        serde_json::from_str(&out).unwrap_or_else(|e| panic!("leon {args:?}: not JSON ({e}): {out}"))
    }

    /// An executable `sh` script named `name` holding `body`.
    pub fn script(&self, name: &str, body: &str) -> PathBuf {
        let dir = self.env.dir.path().join("bin");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    /// A block running a line reader that echoes `GOT: <line>` for each line typed into it; its
    /// first window's only block. Returns (session, block).
    pub fn reader(&self, label: &str) -> (String, String) {
        let b = self.env.ok(&["run", "--label", label, "--", "sh", "-c", READER]);
        self.env.screen_until(&b, "the reader's prompt", |s| s.contains("ready>"));
        let v = self.env.json(&["block", "inspect", &b, "--json"]);
        let w = v["window_id"].as_str().unwrap().to_string();
        let sessions = self.env.json(&["ls", "--json"]);
        let s = sessions
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["session_id"].as_str().unwrap().to_string())
            .find(|s| self.env.view(s)["windows"].as_array().unwrap().iter().any(|x| x["window_id"] == w.as_str()))
            .unwrap();
        (s, b)
    }

    /// How many times `text` shows in block `b`'s screen and scrollback.
    pub fn count(&self, b: &str, text: &str) -> usize {
        self.env.capture(b).matches(text).count()
    }
}

/// Prints a prompt, then `GOT: <line>` for every line typed.
pub const READER: &str = "echo ready\\>; while IFS= read -r l; do echo \"GOT: $l\"; done";

/// Poll `f` every 50 ms until it gives a value; panic after `secs` seconds naming `what`.
pub fn until<T>(what: &str, secs: u64, mut f: impl FnMut() -> Option<T>) -> T {
    let end = Instant::now() + Duration::from_secs(secs);
    loop {
        if let Some(v) = f() {
            return v;
        }
        assert!(Instant::now() < end, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}
