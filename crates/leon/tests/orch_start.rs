//! Agent orchestration criterion 1: `leon agent start` runs the agent with the task and leon's
//! reporting paragraph as its last argument, in a new tab or a split pane, records the caller as
//! the worker's parent, prints the new block ID, and lets a worker start workers of its own.

mod support;

use std::io::Write;
use std::process::Stdio;

use support::orch::{until, Orch};

/// A fake agent: writes each argument it got, NUL-terminated, to `<dir>/argv-<its block>`, then idles.
fn fake_agent(o: &Orch) -> std::path::PathBuf {
    let out = o.env.dir.path().join("out");
    std::fs::create_dir_all(&out).unwrap();
    let body = format!("printf '%s\\0' \"$@\" > '{}/argv-'\"${{LEON_BLOCK#block:}}\"\nexec sleep 300", out.display());
    o.script("fake-agent", &body)
}

/// The argv the fake agent in block `b` got.
fn argv(o: &Orch, b: &str) -> Vec<String> {
    let path = o.env.dir.path().join("out").join(format!("argv-{}", b.trim_start_matches("block:")));
    let raw = until("the agent's argv", 15, || std::fs::read(&path).ok().filter(|r| !r.is_empty()));
    raw.split(|&c| c == 0).filter(|a| !a.is_empty()).map(|a| String::from_utf8_lossy(a).into_owned()).collect()
}

fn workers(o: &Orch, s: &str, b: &str) -> Vec<serde_json::Value> {
    let v = o.json_inside(s, b, &["agent", "ls", "--json"]);
    v["workers"].as_array().unwrap_or_else(|| panic!("no workers list: {v}")).clone()
}

#[test]
fn start_runs_the_agent_with_the_task_and_links_it_to_its_caller() {
    let o = Orch::new();
    let (s, boss) = o.env.session();
    let agent = fake_agent(&o);
    let agent = agent.to_str().unwrap();

    let w1 = o.ok_inside(&s, &boss, &["agent", "start", "--label", "w1", agent, "fix the tests", "--", "--model", "m"]);
    assert!(w1.starts_with("block:"), "prints the new block's ID: {w1:?}");
    let a = argv(&o, &w1);
    assert_eq!(&a[..2], ["--model", "m"], "the agent's own arguments come first: {a:?}");
    assert_eq!(a.len(), 3, "then one prompt argument: {a:?}");
    assert!(a[2].starts_with("fix the tests"), "the prompt starts with the task: {:?}", a[2]);
    assert!(a[2].contains("leon notify"), "leon's paragraph says how to report: {:?}", a[2]);
    let windows = o.env.view(&s)["windows"].as_array().unwrap().len();
    assert_eq!(windows, 2, "a new tab by default");

    // --split puts it beside the caller; the task from a file
    let task = o.env.dir.path().join("task.md");
    std::fs::write(&task, "review the diff\n").unwrap();
    let w2 = o.ok_inside(&s, &boss, &["agent", "start", "--split", "right", "--file", task.to_str().unwrap(), agent]);
    assert!(argv(&o, &w2).last().unwrap().starts_with("review the diff"));
    let boss_window = o.env.json(&["block", "inspect", &boss, "--json"])["window_id"].clone();
    assert_eq!(o.env.json(&["block", "inspect", &w2, "--json"])["window_id"], boss_window, "--split: the caller's tab");

    // -c sets its directory
    let dir = o.env.dir.path().join("proj");
    std::fs::create_dir(&dir).unwrap();
    let w4 = o.ok_inside(&s, &boss, &["agent", "start", "-c", dir.to_str().unwrap(), agent, "t"]);
    argv(&o, &w4);
    let cwd = o.env.json(&["block", "inspect", &w4, "--json"])["cwd"].as_str().unwrap().to_string();
    assert_eq!(std::fs::canonicalize(cwd).unwrap(), std::fs::canonicalize(&dir).unwrap());
    o.env.ok(&["block", "close", &w4]);
    o.ok_inside(&s, &boss, &["inbox"]); // its `closed` report

    // the task from stdin
    let mut child = o
        .inside(&s, &boss)
        .args(["agent", "start", "--label", "w3", agent, "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"from stdin").unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    let w3 = String::from_utf8(out.stdout).unwrap().trim().to_string();
    assert!(argv(&o, &w3).last().unwrap().starts_with("from stdin"));

    let list = workers(&o, &s, &boss);
    let ids: Vec<&str> = list.iter().map(|w| w["block_id"].as_str().unwrap()).collect();
    assert_eq!(ids, [w1.as_str(), w2.as_str(), w3.as_str()], "{list:?}");
    assert_eq!(list[0]["label"], "w1");
    assert_eq!(list[0]["agent"], "fake-agent");
    assert!(list[0]["started_at"].as_u64().is_some_and(|t| t > 1_700_000_000), "{list:?}");
    let text = o.ok_inside(&s, &boss, &["agent", "ls"]);
    assert!(text.contains("w1") && text.contains("fake-agent") && text.contains(&w1), "{text}");
    assert_eq!(o.json_inside(&s, &w1, &["agent", "ls", "--json"])["parent"], boss.as_str(), "the caller is the parent");

    // nesting: a worker's own worker is its child, not the root's
    let sub = o.ok_inside(&s, &w1, &["agent", "start", "--label", "sub", agent, "a sub-task"]);
    let mine: Vec<String> = workers(&o, &s, &w1).iter().map(|w| w["block_id"].as_str().unwrap().to_string()).collect();
    assert_eq!(mine, std::slice::from_ref(&sub));
    assert!(workers(&o, &s, &boss).iter().all(|w| w["block_id"] != sub.as_str()), "only direct children");
}

#[test]
fn start_outside_leon_needs_a_block_to_report_to() {
    let o = Orch::new();
    let agent = fake_agent(&o);
    let out = o.cmd().args(["agent", "start", agent.to_str().unwrap(), "task"]).output().unwrap();
    assert_eq!(out.status.code(), Some(2), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stderr).contains("LEON_BLOCK"), "says how to name the parent");
}
