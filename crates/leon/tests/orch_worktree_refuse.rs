//! Agent worktrees criterion 2: closing a worker whose worktree holds uncommitted work, or commits
//! only its branch has, is refused with a list of what would be lost; pushed work closes cleanly.

mod support;

use support::orch::{git, Orch};

#[test]
fn close_refuses_to_lose_uncommitted_or_unpushed_work() {
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
            "work",
            agent.to_str().unwrap(),
            "t",
        ],
    );
    let path = repo.parent().unwrap().join("proj-worktrees/work");

    std::fs::write(path.join("notes.txt"), "draft\n").unwrap();
    let out = o.run_inside(&s, &boss, &["agent", "close", &w]);
    let err = String::from_utf8_lossy(&out.stderr).to_string();
    assert_eq!(out.status.code(), Some(1), "{err}");
    assert!(err.contains("notes.txt"), "names the uncommitted file: {err}");
    assert!(err.contains("--force"), "says how to remove it anyway: {err}");
    assert!(path.join("notes.txt").exists(), "the file stays");
    assert!(o.env.run(&["block", "inspect", &w]).status.success(), "the worker stays");

    git(&path, &["add", "notes.txt"]);
    git(&path, &["commit", "-q", "-m", "the notes only this branch has"]);
    let out = o.run_inside(&s, &boss, &["agent", "close", &w]);
    let err = String::from_utf8_lossy(&out.stderr).to_string();
    assert_eq!(out.status.code(), Some(1), "{err}");
    assert!(err.contains("the notes only this branch has"), "names the unpushed commit: {err}");
    assert!(path.exists());

    let remote = o.env.dir.path().join("remote.git");
    git(o.env.dir.path(), &["init", "-q", "--bare", remote.to_str().unwrap()]);
    git(&path, &["remote", "add", "origin", remote.to_str().unwrap()]);
    git(&path, &["push", "-q", "origin", "work"]);
    o.ok_inside(&s, &boss, &["agent", "close", &w]);
    assert!(!path.exists(), "pushed: removed");
}

#[test]
fn a_worktree_deleted_by_hand_still_guards_its_branchs_commits() {
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
            "gone",
            agent.to_str().unwrap(),
            "t",
        ],
    );
    let path = repo.parent().unwrap().join("proj-worktrees/gone");
    git(&path, &["commit", "-q", "--allow-empty", "-m", "only on the gone branch"]);
    std::fs::remove_dir_all(&path).unwrap();
    let out = o.run_inside(&s, &boss, &["agent", "close", &w]);
    let err = String::from_utf8_lossy(&out.stderr).to_string();
    assert_eq!(out.status.code(), Some(1), "{err}");
    assert!(err.contains("only on the gone branch"), "names the commit: {err}");
    assert!(!git(&repo, &["branch", "--list", "gone"]).trim().is_empty(), "the branch stays");
}
