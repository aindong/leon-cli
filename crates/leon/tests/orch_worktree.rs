//! Agent worktrees criterion 1: `leon agent start --worktree` gives the worker its own git
//! worktree and branch beside the repository, and `leon agent close` removes both.

mod support;

use support::orch::{git, until, Orch};

#[test]
fn a_worker_gets_a_worktree_and_close_removes_it() {
    let o = Orch::new();
    let repo = o.repo();
    let (s, boss) = o.env.session();
    let agent = o.script("pwd-agent", "pwd; exec sleep 300");
    let dir = repo.to_str().unwrap();
    let w = o.ok_inside(
        &s,
        &boss,
        &["agent", "start", "-c", dir, "--worktree", "--branch", "feat-x", agent.to_str().unwrap(), "t"],
    );
    let path = repo.parent().unwrap().join("proj-worktrees").join("feat-x");
    assert!(git(&repo, &["worktree", "list"]).contains(path.to_str().unwrap()), "a worktree at {}", path.display());
    assert_eq!(git(&path, &["branch", "--show-current"]).trim(), "feat-x");
    o.env.screen_until(&w, "the worker in its worktree", |t| t.contains(path.to_str().unwrap()));
    let team = o.json_inside(&s, &boss, &["agent", "ls", "--json"]);
    assert_eq!(team["workers"][0]["worktree"]["path"], path.to_str().unwrap(), "{team}");
    assert_eq!(team["workers"][0]["worktree"]["branch"], "feat-x");

    o.ok_inside(&s, &boss, &["agent", "close", &w]);
    until("the block closed", 10, || o.env.run(&["block", "inspect", &w]).status.code().filter(|c| *c == 3));
    assert!(!path.exists(), "the worktree is gone");
    assert!(git(&repo, &["branch", "--list", "feat-x"]).trim().is_empty(), "and its branch");

    // a default branch: agent/<label>
    let w2 = o.ok_inside(
        &s,
        &boss,
        &["agent", "start", "-c", dir, "--worktree", "--label", "docs", agent.to_str().unwrap(), "t"],
    );
    assert_eq!(
        git(&repo.parent().unwrap().join("proj-worktrees/agent-docs"), &["branch", "--show-current"]).trim(),
        "agent/docs"
    );
    o.ok_inside(&s, &boss, &["agent", "close", &w2]);
}
