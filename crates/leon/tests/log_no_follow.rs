#[test]
fn server_log_does_not_follow_symlinks() {
    let dir = tempfile::tempdir().unwrap();
    let victim = dir.path().join("victim");
    std::os::unix::fs::symlink(&victim, dir.path().join("server.log")).unwrap();
    let leon = || {
        let mut c = std::process::Command::new(env!("CARGO_BIN_EXE_leon"));
        c.env("LEON_RUNTIME_DIR", dir.path()).env_remove("LEON_AUTOSTART");
        c
    };
    let _ = leon().args(["server", "start"]).output().unwrap();
    let _ = leon().args(["--autostart=false", "server", "stop"]).output();
    assert!(!victim.exists(), "LOG_FOLLOWED_SYMLINK");
}
