//! Stopping the server ends every program it runs (docs/spec.md §1, "stopping ends every program"):
//! the state drops when `serve` returns, and each terminal hangs up its process group.

use std::os::unix::fs::PermissionsExt;
use std::process::Command;
use std::time::{Duration, Instant};

#[test]
fn server_stop_ends_every_program() {
    let dir = tempfile::Builder::new().permissions(std::fs::Permissions::from_mode(0o700)).tempdir().unwrap();
    let leon = |args: &[&str]| {
        let out = Command::new(env!("CARGO_BIN_EXE_leon"))
            .env("LEON_RUNTIME_DIR", dir.path())
            .env("LEON_STATE_DIR", std::path::Path::new(dir.path()).join("state"))
            .env("SHELL", "/bin/sh")
            .env_remove("LEON_SESSION")
            .env_remove("LEON_BLOCK")
            .args(args)
            .output()
            .unwrap();
        assert!(out.status.success(), "leon {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8(out.stdout).unwrap().trim().to_string()
    };
    leon(&["new"]);
    let b = leon(&["run", "--", "sleep", "300"]);
    let info: serde_json::Value = serde_json::from_str(&leon(&["block", "inspect", &b, "--json"])).unwrap();
    let pid = info["pid"].as_i64().unwrap() as i32;
    // SAFETY: signal 0 only checks that the process exists.
    let alive = || unsafe { libc::kill(pid, 0) == 0 };
    assert!(alive());
    leon(&["server", "stop"]);
    let end = Instant::now() + Duration::from_secs(5);
    while alive() && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(!alive(), "sleep {pid} outlived the server");
}
