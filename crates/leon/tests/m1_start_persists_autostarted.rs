//! `server start` must make an already-running autostarted server persistent.

use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Output};
use std::time::Duration;

struct Env {
    dir: tempfile::TempDir,
}

impl Env {
    fn cmd(&self) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_leon"));
        c.env("LEON_RUNTIME_DIR", self.dir.path())
            .env("LEON_STATE_DIR", std::path::Path::new(self.dir.path()).join("state"))
            .env("LEON_IDLE_EXIT_SECS", "1")
            .env_remove("LEON_SESSION")
            .env_remove("LEON_AUTOSTART");
        c
    }
    fn run(&self, args: &[&str]) -> Output {
        self.cmd().args(args).output().unwrap()
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = self.run(&["--autostart=false", "server", "stop"]);
    }
}

#[test]
fn server_start_makes_an_autostarted_server_persistent() {
    let env =
        Env { dir: tempfile::Builder::new().permissions(std::fs::Permissions::from_mode(0o700)).tempdir().unwrap() };
    assert!(env.run(&["ls"]).status.success(), "ls autostarts a server");
    let started = env.run(&["server", "start"]);
    assert!(started.status.success(), "{}", String::from_utf8_lossy(&started.stderr));
    std::thread::sleep(Duration::from_secs(3)); // well past LEON_IDLE_EXIT_SECS=1
    let st = env.run(&["--autostart=false", "server", "status", "--json"]);
    assert_eq!(
        st.status.code(),
        Some(0),
        "server start promised a persistent server, but it idle-exited: {}",
        String::from_utf8_lossy(&st.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&st.stdout).unwrap();
    assert_eq!(v["autostarted"], false, "after server start the server must no longer be autostarted");
}
