//! Agent orchestration criterion 3: leon types "You have N leon messages. Run `leon inbox`." into
//! the orchestrator's prompt only once it reports its turn ended, one line for every report that
//! came meanwhile, and never into a pane that had input in the last 3 s.

mod support;

use std::time::{Duration, Instant};

use support::orch::{until, Orch};

const NOTICE: &str = "GOT: You have 2 leon messages. Run leon inbox to read them.";

#[test]
fn the_notice_waits_for_an_idle_orchestrator_and_coalesces() {
    let o = Orch::new();
    // the orchestrator: a line reader standing in for an agent's prompt
    let (s, boss) = o.reader("boss");
    o.env.ok(&["notify", "-b", &boss, "--state", "working"]);
    let agent = o.script("idle-agent", "exec sleep 300");
    let w1 = o.ok_inside(&s, &boss, &["agent", "start", agent.to_str().unwrap(), "one"]);
    let w2 = o.ok_inside(&s, &boss, &["agent", "start", agent.to_str().unwrap(), "two"]);
    o.env.ok(&["notify", "-b", &w1, "--state", "done", "--body", "first"]);
    o.env.ok(&["notify", "-b", &w2, "--state", "done", "--body", "second"]);

    std::thread::sleep(Duration::from_secs(4));
    assert_eq!(o.count(&boss, "leon messages"), 0, "nothing typed while it works:\n{}", o.env.capture(&boss));

    o.env.ok(&["notify", "-b", &boss, "--state", "done"]); // its turn ended
    until("the notice", 15, || (o.count(&boss, NOTICE) == 1).then_some(()));
    std::thread::sleep(Duration::from_secs(8));
    assert_eq!(o.count(&boss, NOTICE), 1, "one notice for both, typed once:\n{}", o.env.capture(&boss));
    o.ok_inside(&s, &boss, &["inbox"]);

    // a person typed a moment ago: the next notice waits for 3 s of quiet
    o.env.ok(&["send", "-b", &boss, "typed by hand"]);
    o.env.ok(&["send-key", "-b", &boss, "enter"]);
    let typed = Instant::now();
    o.env.ok(&["notify", "-b", &w1, "--state", "error", "--body", "broke"]);
    o.env.ok(&["notify", "-b", &w2, "--state", "done", "--body", "again"]);
    until("the second notice", 15, || (o.count(&boss, NOTICE) == 2).then_some(()));
    let waited = typed.elapsed();
    assert!(waited >= Duration::from_millis(2900), "typed {waited:?} after the person's input");
    assert_eq!(o.count(&boss, "GOT: typed by hand"), 1, "the person's line stayed whole");
}

#[test]
fn a_pane_that_never_reported_is_never_typed_into() {
    let o = Orch::new();
    let (s, shell) = o.reader("plain-shell");
    let agent = o.script("idle-agent", "exec sleep 300");
    let w = o.ok_inside(&s, &shell, &["agent", "start", agent.to_str().unwrap(), "t"]);
    o.env.ok(&["notify", "-b", &w, "--state", "done", "--body", "finished"]);
    std::thread::sleep(Duration::from_secs(5));
    assert_eq!(o.count(&shell, "leon messages"), 0, "no agent hook said it is idle");
    assert_eq!(o.json_inside(&s, &shell, &["inbox", "--json"]).as_array().unwrap().len(), 1, "the report waits");
}
