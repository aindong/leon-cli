//! M14 acceptance (.agent/plans/2026-11-15-m14-resume-after-restart.md, criterion 9):
//! `leon.set("resume", …)` in init.lua, shown by `leon config check`; bad names and values are errors.

use std::os::unix::fs::PermissionsExt;
use std::process::Command;

fn check(lua: &str) -> (i32, String) {
    let rt = tempfile::Builder::new().permissions(std::fs::Permissions::from_mode(0o700)).tempdir().unwrap();
    let cfg = rt.path().join("init.lua");
    std::fs::write(&cfg, lua).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_leon"))
        .args(["config", "check", cfg.to_str().unwrap()])
        .env("LEON_RUNTIME_DIR", rt.path())
        .env("LEON_AUTOSTART", "0")
        .output()
        .unwrap();
    (o.status.code().unwrap(), String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr))
}

#[test]
fn the_resume_setting_loads_and_is_checked() {
    for policy in ["ask", "always", "never"] {
        let (code, out) = check(&format!("leon.set(\"resume\", \"{policy}\")\n"));
        assert_eq!(code, 0, "{out}");
        assert!(out.lines().any(|l| l == format!("resume: {policy}")), "{out}");
    }
    let (code, out) = check("leon.bind(\"cmd+t\", \"client.tab.new\")\n");
    assert_eq!(code, 0, "{out}");
    assert!(!out.contains("resume:"), "only shown when set: {out}");

    for bad in ["leon.set(\"resume\", \"sometimes\")", "leon.set(\"resum\", \"ask\")", "leon.set(\"resume\", true)"] {
        let (code, out) = check(bad);
        assert_eq!(code, 1, "{bad}: {out}");
        assert!(out.contains("leon.set"), "the diagnostic names the call: {out}");
    }
}
