use std::os::unix::fs::PermissionsExt;
use std::process::Command;

#[test]
fn server_start_makes_its_runtime_dir_private() {
    let parent = tempfile::tempdir().unwrap();
    let dir = parent.path().join("run");
    let leon = || {
        let mut c = Command::new(env!("CARGO_BIN_EXE_leon"));
        c.env("LEON_RUNTIME_DIR", &dir)
            .env("LEON_STATE_DIR", std::path::Path::new(&dir).join("state"))
            .env_remove("LEON_AUTOSTART");
        c
    };
    let out = leon().args(["server", "start"]).output().unwrap();
    let mode = std::fs::metadata(&dir).map(|m| m.permissions().mode() & 0o777);
    let _ = leon().args(["--autostart=false", "server", "stop"]).output();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(mode.unwrap(), 0o700, "RUNTIME_DIR_NOT_PRIVATE");
}
