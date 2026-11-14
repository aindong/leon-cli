//! Review fix: `setup --force` over a SKILL.md that is a symlink replaces the link, never the
//! unrelated file it points to.

use std::os::unix::fs::PermissionsExt;
use std::process::Command;

#[test]
fn force_replacing_a_symlinked_foreign_skill_leaves_the_link_target_alone() {
    let root = tempfile::tempdir().unwrap();
    let rt = tempfile::Builder::new().permissions(std::fs::Permissions::from_mode(0o700)).tempdir().unwrap();
    let (home, claude, codex) = (root.path().join("home"), root.path().join("claude"), root.path().join("codex"));
    std::fs::create_dir_all(&home).unwrap();
    let other = root.path().join("dotfiles/skills/mine/SKILL.md");
    std::fs::create_dir_all(other.parent().unwrap()).unwrap();
    let mine = "---\nname: mine\n---\nmy own skill\n";
    std::fs::write(&other, mine).unwrap();
    let skill = claude.join("skills/leon/SKILL.md");
    std::fs::create_dir_all(skill.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&other, &skill).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_leon"))
        .args(["skill", "setup", "--agent", "claude", "--force"])
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
    assert_eq!(
        std::fs::read_to_string(&other).unwrap(),
        mine,
        "setup --force overwrote the symlink target, an unrelated file"
    );
    assert!(!std::fs::symlink_metadata(&skill).unwrap().file_type().is_symlink(), "the link was replaced by our skill");
    let backup = claude.join("skills/leon/SKILL.md.leon-backup");
    assert_eq!(std::fs::read_to_string(backup).unwrap(), mine, "the replaced skill's contents are backed up");
}
