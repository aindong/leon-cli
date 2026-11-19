//! Agent worktrees criterion 5: the worktree record survives a server restart, and a worker
//! closed some other way keeps its worktree on disk, which its `closed` report names.

mod support;

use support::orch::{private, until, Orch};

#[test]
fn the_record_survives_a_restart_and_another_close_keeps_the_worktree() {
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
            "kept",
            agent.to_str().unwrap(),
            "t",
        ],
    );
    let path = repo.parent().unwrap().join("proj-worktrees/kept");

    o.env.ok(&["server", "stop"]);
    let out = private(o.env.cmd(), &o.home).args(["server", "start"]).output().unwrap();
    assert!(out.status.success());
    let team = o.json_inside(&s, &boss, &["agent", "ls", "--json"]);
    assert_eq!(team["workers"][0]["worktree"]["path"], path.to_str().unwrap(), "{team}");

    o.env.ok(&["block", "close", &w]);
    let inbox = until("the closed report", 10, || {
        let v = o.json_inside(&s, &boss, &["inbox", "--json"]);
        (!v.as_array().unwrap().is_empty()).then_some(v)
    });
    assert_eq!(inbox[0]["state"], "closed");
    assert!(inbox[0]["body"].as_str().unwrap_or("").contains(path.to_str().unwrap()), "names the worktree: {inbox}");
    assert!(path.exists(), "a worktree is never removed without `agent close`");
}
