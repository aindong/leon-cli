use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Output};

struct Env(tempfile::TempDir);
impl Env {
    fn new() -> Self {
        Env(tempfile::Builder::new().permissions(std::fs::Permissions::from_mode(0o700)).tempdir().unwrap())
    }
    fn run(&self, env: &[(&str, &str)], args: &[&str]) -> Output {
        let mut c = Command::new(env!("CARGO_BIN_EXE_leon"));
        c.env("LEON_RUNTIME_DIR", self.0.path())
            .env("LEON_STATE_DIR", std::path::Path::new(self.0.path()).join("state"))
            .env("SHELL", "/bin/sh");
        for k in ["LEON_SESSION", "LEON_BLOCK", "LEON_SERVER", "LEON_AUTOSTART", "LEON_IDLE_EXIT_SECS"] {
            c.env_remove(k);
        }
        c.envs(env.iter().copied()).args(args).output().unwrap()
    }
    fn ok(&self, env: &[(&str, &str)], args: &[&str]) -> String {
        let o = self.run(env, args);
        assert!(o.status.success(), "leon {args:?}: exit {:?} {}", o.status.code(), String::from_utf8_lossy(&o.stderr));
        String::from_utf8(o.stdout).unwrap().trim().to_string()
    }
    fn first_block(&self, s: &str) -> String {
        let v: serde_json::Value = serde_json::from_str(&self.ok(&[], &["-s", s, "block", "ls", "--json"])).unwrap();
        v[0]["block_id"].as_str().unwrap().to_string()
    }
}
impl Drop for Env {
    fn drop(&mut self) {
        let _ = self.run(&[], &["--autostart=false", "server", "stop"]);
    }
}

#[test]
fn naming_a_session_beats_leon_block() {
    let env = Env::new();
    let s1 = env.ok(&[], &["new"]);
    let s2 = env.ok(&[], &["new"]);
    let b1 = env.first_block(&s1);
    let out = env.run(&[("LEON_BLOCK", b1.as_str())], &["-s", &s2, "block", "inspect", "--json"]);
    assert!(
        out.status.success(),
        "-s lost to $LEON_BLOCK: exit {:?} {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn positions_count_in_leon_session() {
    let env = Env::new();
    let _s1 = env.ok(&[], &["new"]);
    let s2 = env.ok(&[], &["new"]);
    let b2 = env.first_block(&s2);
    let out = env.ok(&[("LEON_SESSION", s2.as_str())], &["block", "inspect", "1", "--json"]);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["block_id"].as_str(), Some(b2.as_str()), "position 1 resolved outside $LEON_SESSION");
}
