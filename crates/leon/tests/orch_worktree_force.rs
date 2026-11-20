//! Agent worktrees criterion 3: `leon agent close --force` removes a worktree whatever it holds.

mod support;

use support::orch::{git, Orch};

#[test]
fn force_removes_a_worktree_with_work_in_it() {
    let o = Orch::new();
    let repo = o.repo();
    let (s, boss) = o.env.session();
    let agent = o.script("idle-agent", "exec sleep 300");
    let w = o.ok_inside(
        &s,
        &boss,
        &[
            "agent",
            "start",
            "-c",
            repo.to_str().unwrap(),
            "--worktree",
            "--branch",
            "scratch",
            agent.to_str().unwrap(),
            "t",
        ],
    );
    let path = repo.parent().unwrap().join("proj-worktrees/scratch");
    std::fs::write(path.join("a.txt"), "a\n").unwrap();
    git(&path, &["add", "a.txt"]);
    git(&path, &["commit", "-q", "-m", "unpushed"]);
    std::fs::write(path.join("b.txt"), "b\n").unwrap();
    o.ok_inside(&s, &boss, &["agent", "close", &w, "--force"]);
    assert!(!path.exists());
    assert!(git(&repo, &["branch", "--list", "scratch"]).trim().is_empty());
}
