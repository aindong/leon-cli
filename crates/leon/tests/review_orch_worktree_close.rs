//! Review (PR 2): `leon agent close` never removes what leon didn't make or what holds work: a
//! record anyone could have written, ignored files, a locked worktree; a renamed branch still
//! closes cleanly.

mod support;

use support::orch::{git, Orch};

fn start(o: &Orch, repo: &std::path::Path, s: &str, boss: &str, branch: &str) -> (String, std::path::PathBuf) {
    let agent = o.script("idle-agent", "exec sleep 300");
    let args = [
        "agent",
        "start",
        "-c",
        repo.to_str().unwrap(),
        "--worktree",
        "--branch",
        branch,
        agent.to_str().unwrap(),
        "t",
    ];
    let w = o.ok_inside(s, boss, &args);
    (w, repo.parent().unwrap().join("proj-worktrees").join(branch))
}

#[test]
fn close_never_deletes_a_branch_leon_did_not_make() {
    let o = Orch::new();
    let repo = o.repo();
    git(&repo, &["switch", "-q", "-c", "precious"]);
    git(&repo, &["commit", "-q", "--allow-empty", "-m", "only on precious"]);
    git(&repo, &["switch", "-q", "main"]);
    let (s, boss) = o.env.session();
    let agent = o.script("idle-agent", "exec sleep 300");
    let w = o.ok_inside(&s, &boss, &["agent", "start", agent.to_str().unwrap(), "t"]);
    for path in [o.env.dir.path().join("no-such-worktree"), repo.parent().unwrap().join("proj-worktrees/precious")] {
        let link = serde_json::json!({ "block_id": w, "parent_id": boss, "agent": "x",
            "worktree": { "path": path, "branch": "precious", "repo": repo } });
        o.env.ok(&["api", "call", "agent.link", &link.to_string()]);
        for force in [false, true] {
            let mut args = vec!["agent", "close", &w];
            if force {
                args.push("--force");
            }
            let out = o.run_inside(&s, &boss, &args);
            assert_eq!(out.status.code(), Some(1), "{}", String::from_utf8_lossy(&out.stderr));
            assert!(String::from_utf8_lossy(&out.stderr).contains("won't remove"));
        }
    }
    assert!(
        !git(&repo, &["branch", "--list", "precious"]).trim().is_empty(),
        "BUG: a branch leon never made was deleted"
    );
    assert!(o.env.run(&["block", "inspect", &w]).status.success(), "nothing was closed");
}

#[test]
fn ignored_files_and_a_lock_hold_a_worktree_too() {
    let o = Orch::new();
    let repo = o.repo();
    std::fs::write(repo.join(".gitignore"), ".env\n").unwrap();
    git(&repo, &["add", ".gitignore"]);
    git(&repo, &["commit", "-q", "-m", "ignore .env"]);
    let (s, boss) = o.env.session();
    let (w, path) = start(&o, &repo, &s, &boss, "ign");
    std::fs::write(path.join(".env"), "TOKEN=fake-test-value\n").unwrap();
    let out = o.run_inside(&s, &boss, &["agent", "close", &w]);
    let err = String::from_utf8_lossy(&out.stderr).to_string();
    assert_eq!(out.status.code(), Some(1), "{err}");
    assert!(err.contains(".env") && path.join(".env").exists(), "BUG: an ignored file went without --force: {err}");

    std::fs::remove_file(path.join(".env")).unwrap();
    git(&repo, &["worktree", "lock", path.to_str().unwrap()]);
    let out = o.run_inside(&s, &boss, &["agent", "close", &w]);
    let err = String::from_utf8_lossy(&out.stderr).to_string();
    assert_eq!(out.status.code(), Some(1), "{err}");
    assert!(err.contains("locked") && path.exists(), "{err}");
    assert!(o.env.run(&["block", "inspect", &w]).status.success(), "nothing was closed");
}

#[test]
fn a_renamed_branch_is_left_alone_and_the_report_is_neutral() {
    let o = Orch::new();
    let repo = o.repo();
    let (s, boss) = o.env.session();
    let (w, path) = start(&o, &repo, &s, &boss, "orig");
    git(&path, &["branch", "-m", "renamed"]);
    let out = o.run_inside(&s, &boss, &["agent", "close", &w]);
    // a renamed branch isn't the one leon made: it refuses, and the work stays on `renamed`
    let err = String::from_utf8_lossy(&out.stderr).to_string();
    assert_eq!(out.status.code(), Some(1), "{err}");
    assert!(!git(&repo, &["branch", "--list", "renamed"]).trim().is_empty());

    let (w2, path2) = start(&o, &repo, &s, &boss, "plain");
    o.ok_inside(&s, &boss, &["inbox"]);
    o.ok_inside(&s, &boss, &["agent", "close", &w2]);
    assert!(!path2.exists());
    let inbox = support::orch::until("the closed report", 10, || {
        let v = o.json_inside(&s, &boss, &["inbox", "--json"]);
        (!v.as_array().unwrap().is_empty()).then_some(v)
    });
    assert!(!inbox[0]["body"].as_str().unwrap_or("").contains("stays"), "BUG: says a removed worktree stays: {inbox}");
}
