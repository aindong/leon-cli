//! Review fix: a user's own SessionStart hook that merely mentions `leon skill hook` is theirs, so
//! setup and remove leave it alone.

use std::os::unix::fs::PermissionsExt;
use std::process::Command;

#[test]
fn setup_and_remove_keep_a_user_hook_that_mentions_leon_skill_hook() {
    let root = tempfile::tempdir().unwrap();
    let rt = tempfile::Builder::new().permissions(std::fs::Permissions::from_mode(0o700)).tempdir().unwrap();
    let (home, claude, codex) = (root.path().join("home"), root.path().join("claude"), root.path().join("codex"));
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&claude).unwrap();
    let mine = r#"{"hooks":{"SessionStart":[{"hooks":[{"type":"command","command":"~/bin/load-context.sh; leon skill hook --legacy"}]}]}}"#;
    std::fs::write(claude.join("settings.json"), mine).unwrap();
    let leon = |args: &[&str]| {
        let o = Command::new(env!("CARGO_BIN_EXE_leon"))
            .args(args)
            .env("HOME", &home)
            .env("CLAUDE_CONFIG_DIR", &claude)
            .env("CODEX_HOME", &codex)
            .env("LEON_RUNTIME_DIR", rt.path())
            .env("LEON_AUTOSTART", "0")
            .env_remove("LEON_SESSION")
            .env_remove("LEON_BLOCK")
            .output()
            .unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    };
    leon(&["skill", "setup", "--agent", "claude"]);
    leon(&["skill", "remove", "--agent", "claude"]);
    let now = std::fs::read_to_string(claude.join("settings.json")).unwrap_or_default();
    assert!(now.contains("load-context.sh"), "user's own SessionStart hook was deleted: {now:?}");
}
