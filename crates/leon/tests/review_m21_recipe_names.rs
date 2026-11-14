//! Review proof: different folder spellings must not silently collapse after label trimming.
mod support;

use std::path::Path;
use std::process::Command;

#[test]
fn normalized_worktree_names_are_checked_before_creating_any_session() {
    let env = support::Env::new();
    let a = env.dir.path().join("project");
    let b = env.dir.path().join(" project ");
    std::fs::create_dir(&a).unwrap();
    let git = |args: &[&str]| {
        let out = Command::new("git").arg("-C").arg(&a).args(args).output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    };
    git(&["init", "-q"]);
    git(&["-c", "user.name=Test", "-c", "user.email=test@example.test", "commit", "--allow-empty", "-qm", "initial"]);
    git(&["worktree", "add", "-qb", "feature", b.to_str().unwrap()]);
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/projects.lua");
    let out =
        env.run(&["do", "--for", "10s", script.to_str().unwrap(), &format!("dir={}", a.display()), "worktrees=true"]);
    assert!(!out.status.success(), "normalized worktree names must fail, not return the same session twice");
    assert!(String::from_utf8_lossy(&out.stderr).contains("share a folder name"));
    assert_eq!(env.json(&["ls", "--json"]), serde_json::json!([]));
}
