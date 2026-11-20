//! Agent orchestration criterion 7: an orchestrator that closes leaves its workers running and
//! unlinked; a worker whose program exits reports `exited` and leaves the team.

mod support;

use support::orch::{until, Orch};

#[test]
fn closing_the_orchestrator_unlinks_its_workers_and_keeps_them_running() {
    let o = Orch::new();
    let (s, boss) = o.reader("boss");
    let agent = o.script("idle-agent", "exec sleep 300");
    let w = o.ok_inside(&s, &boss, &["agent", "start", "--label", "w", agent.to_str().unwrap(), "t"]);
    assert_eq!(o.json_inside(&s, &w, &["agent", "ls", "--json"])["parent"], boss.as_str());
    o.env.ok(&["notify", "-b", &w, "--state", "done", "--body", "unread"]);

    o.env.ok(&["block", "close", &boss]);
    until("the worker unlinked", 10, || {
        o.json_inside(&s, &w, &["agent", "ls", "--json"])["parent"].is_null().then_some(())
    });
    let v = o.env.json(&["block", "inspect", &w, "--json"]);
    assert!(v["exit_code"].is_null(), "the worker still runs: {v}");
    // its reports now go nowhere, and nothing fails
    o.env.ok(&["notify", "-b", &w, "--state", "error", "--body", "nobody listens"]);
}

#[test]
fn a_worker_that_exits_reports_it_and_leaves_the_team() {
    let o = Orch::new();
    let (s, boss) = o.env.session();
    let agent = o.script("short-agent", "echo working; sleep 1; exit 3");
    let w = o.ok_inside(&s, &boss, &["agent", "start", "--label", "short", agent.to_str().unwrap(), "t"]);
    let inbox = until("the exit report", 15, || {
        let v = o.json_inside(&s, &boss, &["inbox", "--json"]);
        (!v.as_array().unwrap().is_empty()).then_some(v)
    });
    let r = &inbox[0];
    assert_eq!((r["block_id"].as_str(), r["state"].as_str()), (Some(w.as_str()), Some("exited")), "{inbox}");
    assert!(r["body"].as_str().unwrap().contains('3'), "the exit status: {r}");
    assert_eq!(r["label"], "short");
    let team = o.json_inside(&s, &boss, &["agent", "ls", "--json"]);
    assert_eq!(team["workers"], serde_json::json!([]), "{team}");
}
