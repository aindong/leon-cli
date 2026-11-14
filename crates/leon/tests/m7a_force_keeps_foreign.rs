//! `setup --force` must never destroy a foreign SKILL.md, even when an older SKILL.md.leon-backup exists.

use std::os::unix::fs::PermissionsExt;
use std::process::Command;

#[test]
fn force_over_a_foreign_skill_keeps_it_even_with_an_old_backup() {
    let rt = tempfile::Builder::new().permissions(std::fs::Permissions::from_mode(0o700)).tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let (home, claude, codex) = (root.path().join("home"), root.path().join("claude"), root.path().join("codex"));
    std::fs::create_dir_all(&home).unwrap();
    let dir = claude.join("skills/leon");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("SKILL.md.leon-backup"),
        "---\nname: leon\n---\nskill A, backed up by an earlier --force\n",
    )
    .unwrap();
    let foreign_b = "---\nname: leon\ndescription: skill B\n---\nB's only copy\n";
    std::fs::write(dir.join("SKILL.md"), foreign_b).unwrap();

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
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));

    let files: Vec<String> =
        std::fs::read_dir(&dir).unwrap().filter_map(|e| std::fs::read_to_string(e.unwrap().path()).ok()).collect();
    assert!(
        files.iter().any(|t| *t == foreign_b),
        "foreign SKILL.md destroyed by --force: no file under {} holds it",
        dir.display()
    );
    assert!(files.iter().any(|t| t.contains("skill A")), "the older backup was lost");
}
