//! Security review: setup that creates an agent's home dir keeps it private.
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

#[test]
fn setup_from_path_creates_private_agent_dirs() {
    let root = tempfile::tempdir().unwrap();
    let (home, bin) = (root.path().join("home"), root.path().join("bin"));
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&bin).unwrap();
    for n in ["claude", "codex"] {
        std::fs::write(bin.join(n), "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(bin.join(n), std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let rt = tempfile::Builder::new().permissions(std::fs::Permissions::from_mode(0o700)).tempdir().unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_leon"))
        .args(["skill", "setup"])
        .env("HOME", &home)
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .env("LEON_RUNTIME_DIR", rt.path())
        .env("LEON_STATE_DIR", rt.path().join("state"))
        .env("LEON_AUTOSTART", "0")
        .env_remove("CLAUDE_CONFIG_DIR")
        .env_remove("CODEX_HOME")
        .env_remove("LEON_SESSION")
        .env_remove("LEON_BLOCK")
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    for d in [".claude", ".codex"] {
        let mode = std::fs::metadata(home.join(d)).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode & 0o077, 0, "{d} created {mode:o}: readable by other users");
    }
}
