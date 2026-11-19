//! Agent worktrees criterion 4: `--worktree` outside a git checkout is a clear error.

mod support;

use support::orch::Orch;

#[test]
fn a_worktree_needs_a_git_checkout() {
    let o = Orch::new();
    let (s, boss) = o.env.session();
    let plain = o.env.dir.path().join("plain");
    std::fs::create_dir(&plain).unwrap();
    let agent = o.script("idle-agent", "exec sleep 300");
    let out = o.run_inside(
        &s,
        &boss,
        &["agent", "start", "-c", plain.to_str().unwrap(), "--worktree", "--branch", "x", agent.to_str().unwrap(), "t"],
    );
    let err = String::from_utf8_lossy(&out.stderr).to_string();
    assert_eq!(out.status.code(), Some(1), "{err}");
    assert!(err.contains("not in a git repository"), "{err}");
    assert_eq!(o.env.view(&s)["windows"].as_array().unwrap().len(), 1, "no worker started");
}
