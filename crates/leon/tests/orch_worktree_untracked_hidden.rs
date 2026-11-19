//! Review: `leon agent close` sees untracked files even when git config hides them from
//! `git status`, so it never removes them without `--force`.

mod support;

use support::orch::{git, Orch};

#[test]
fn close_refuses_untracked_work_with_show_untracked_files_off() {
    let o = Orch::new();
    let repo = o.repo();
    git(&repo, &["config", "status.showUntrackedFiles", "no"]);
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
            "work",
            agent.to_str().unwrap(),
            "t",
        ],
    );
    let path = repo.parent().unwrap().join("proj-worktrees/work");
    std::fs::write(path.join("notes.txt"), "the only copy\n").unwrap();

    let out = o.run_inside(&s, &boss, &["agent", "close", &w]);
    let err = String::from_utf8_lossy(&out.stderr).to_string();
    let kept = path.join("notes.txt").exists();
    let alive = o.env.run(&["block", "inspect", &w]).status.success();
    assert!(
        kept && alive && out.status.code() == Some(1) && err.contains("notes.txt"),
        "close lost or ignored untracked work: kept={kept} alive={alive} code={:?} err={err}",
        out.status.code()
    );
}
