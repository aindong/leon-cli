//! Review: an orchestrator agent that quit back to its shell must not get the notice typed into
//! that shell, where it would run as a command.

mod support;

use std::time::Duration;

use support::orch::{until, Orch};

#[test]
fn no_notice_is_typed_into_the_shell_an_orchestrator_left() {
    let o = Orch::new();
    // the orchestrator's pane: a person's interactive shell
    let boss = o.env.ok(&["run", "--label", "boss", "--", "sh", "-c", "PS1='shell> ' exec sh -i"]);
    o.env.screen_until(&boss, "the shell prompt", |s| s.contains("shell>"));
    let s = o.env.json(&["ls", "--json"])[0]["session_id"].as_str().unwrap().to_string();

    let worker = o.script("idle-agent", "exec sleep 300");
    let wb = o.ok_inside(&s, &boss, &["agent", "start", worker.to_str().unwrap(), "task"]);

    // the orchestrator agent runs in that shell (`cat` stands in), and its Stop hook says its turn ended
    o.env.ok(&["send", "-b", &boss, "cat"]);
    o.env.ok(&["send-key", "-b", &boss, "enter"]);
    until("cat in front", 10, || {
        (o.env.json(&["block", "inspect", &boss, "--json"])["foreground"] == "cat").then_some(())
    });
    o.env.ok(&["notify", "-b", &boss, "--state", "done"]);
    // then the person quits the agent: the shell is back in front
    o.env.ok(&["send-key", "-b", &boss, "ctrl+d"]);
    until("the shell's prompt again", 10, || (o.count(&boss, "shell>") >= 2).then_some(()));

    // a worker finishes later
    o.env.ok(&["notify", "-b", &wb, "--state", "done", "--body", "finished"]);
    std::thread::sleep(Duration::from_secs(7));
    assert_eq!(o.count(&boss, "leon message"), 0, "typed into the shell the agent left:\n{}", o.env.capture(&boss));
    assert_eq!(o.json_inside(&s, &boss, &["inbox", "--json"]).as_array().unwrap().len(), 1, "the report waits");
}
