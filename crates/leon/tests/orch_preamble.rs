//! A worker whose agent has leon's hooks reports by its last message, so finishing or asking costs
//! the person no extra approval for a `leon notify` command; an agent without them is told to run
//! `leon notify`. One unread report reads "to read it".

mod support;

use std::time::Duration;

use support::orch::{until, Orch};

/// The prompt (last argument) the worker named `name` got.
fn prompt(o: &Orch, s: &str, boss: &str, name: &str) -> String {
    let out = o.env.dir.path().join(format!("prompt-{name}"));
    let agent = o.script(
        name,
        &format!("for a in \"$@\"; do last=$a; done; printf '%s' \"$last\" > '{}'; exec sleep 300", out.display()),
    );
    let _ = std::fs::remove_file(&out); // a worker of the same name ran before
    o.ok_inside(s, boss, &["agent", "start", agent.to_str().unwrap(), "the task"]);
    until("the worker's prompt", 15, || std::fs::read_to_string(&out).ok().filter(|p| !p.is_empty()))
}

#[test]
fn hooked_agents_report_by_their_last_message() {
    let o = Orch::new();
    let (s, boss) = o.env.session();
    let without = prompt(&o, &s, &boss, "claude");
    assert!(without.contains("leon notify --state done"), "no hooks yet: report with leon notify: {without}");

    let out = o.cmd().args(["skill", "setup", "--agent", "claude"]).output().unwrap();
    assert!(out.status.success());
    let with = prompt(&o, &s, &boss, "claude");
    assert!(with.contains("last message"), "with hooks: the last message is the report: {with}");
    assert!(!with.contains("leon notify"), "no command to approve: {with}");
    let other = prompt(&o, &s, &boss, "my-agent");
    assert!(other.contains("leon notify --state done"), "an agent leon has no hooks for: {other}");
}

#[test]
fn one_report_reads_it() {
    let o = Orch::new();
    let (s, boss) = o.reader("boss");
    o.env.ok(&["notify", "-b", &boss, "--state", "working"]);
    let agent = o.script("idle-agent", "exec sleep 300");
    let w = o.ok_inside(&s, &boss, &["agent", "start", agent.to_str().unwrap(), "t"]);
    o.env.ok(&["notify", "-b", &w, "--state", "done", "--body", "finished"]);
    o.env.ok(&["notify", "-b", &boss, "--state", "done"]);
    until("the notice", 15, || {
        (o.count(&boss, "GOT: You have 1 leon message. Run leon inbox to read it.") == 1).then_some(())
    });
    std::thread::sleep(Duration::from_millis(100));
}
