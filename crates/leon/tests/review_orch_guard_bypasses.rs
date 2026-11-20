//! Review (security): ways an agent in a leon pane tried to answer a permission prompt anyway. A
//! block that reported one stays held until a person types there; programs inside leon can't
//! type into it, change its status from outside it, or run config actions meanwhile.

mod support;

use std::path::Path;
use std::time::Duration;

use support::orch::{until, Orch, READER};

/// A worker held for a person: a line reader whose program reported a permission prompt.
fn held_worker(o: &Orch) -> String {
    let (_, w) = o.reader("worker");
    o.env.ok(&["notify", "-b", &w, "--state", "blocked", "--kind", "permission", "--body", "Bash: rm -rf build"]);
    w
}

/// Run `body` as an agent in a pane beside `w` (which keeps the focus), and wait for `done`.
fn agent_beside(o: &Orch, w: &str, name: &str, body: &str, done: &Path) {
    let agent = o.script(name, &format!("{body}\nexec sleep 300"));
    o.env.ok(&["split", "-b", w, "--focus=false", "--", agent.to_str().unwrap()]);
    until(name, 30, || done.exists().then_some(()));
    std::thread::sleep(Duration::from_millis(500));
}

#[test]
fn an_orphaned_job_is_still_inside_leon() {
    let o = Orch::new();
    let w = held_worker(&o);
    let done = o.env.dir.path().join("orphan-done");
    // the outer subshell exits at once: the job is handed to pid 1, but keeps the pane's session
    let body =
        format!("( (sleep 1; leon send -b {w} orphan; leon send-key -b {w} enter; touch '{}') & )", done.display());
    agent_beside(&o, &w, "orphaning-agent", &body, &done);
    assert_eq!(o.count(&w, "GOT: orphan"), 0, "BYPASS: an orphaned job typed:\n{}", o.env.capture(&w));
}

#[test]
fn a_deep_process_tree_is_still_inside_leon() {
    let o = Orch::new();
    let w = held_worker(&o);
    let done = o.env.dir.path().join("deep-done");
    let deep = o.script(
        "deep",
        &format!(
            "n=${{1:-70}}\nif [ \"$n\" -gt 0 ]; then \"$0\" $((n - 1)); exit $?; fi\n\
             leon send -b {w} deep; leon send-key -b {w} enter; touch '{}'",
            done.display()
        ),
    );
    agent_beside(&o, &w, "deep-agent", deep.to_str().unwrap(), &done);
    assert_eq!(o.count(&w, "GOT: deep"), 0, "BYPASS: 70 generations deep typed:\n{}", o.env.capture(&w));
}

#[test]
fn output_written_to_its_terminal_does_not_lift_the_hold() {
    let o = Orch::new();
    let w = held_worker(&o);
    let pid = o.env.json(&["block", "inspect", &w, "--json"])["pid"].as_i64().unwrap();
    let done = o.env.dir.path().join("tty-done");
    let body = format!(
        "t=$(ps -o tty= -p {pid} 2>/dev/null | tr -d ' '); if [ -n \"$t\" ]; then t=/dev/$t; else t=$(readlink /proc/{pid}/fd/0); fi\n\
         printf '\\033]7501;state=working\\007' > \"$t\"\nsleep 1\n\
         leon send -b {w} tty; leon send-key -b {w} enter; touch '{}'",
        done.display()
    );
    agent_beside(&o, &w, "tty-agent", &body, &done);
    assert_eq!(o.count(&w, "GOT: tty"), 0, "BYPASS: lifted by writing to the pane's terminal:\n{}", o.env.capture(&w));
}

#[test]
fn an_agent_cannot_lift_and_answer_its_own_prompt() {
    let o = Orch::new();
    let done = o.env.dir.path().join("self-done");
    let worker = format!(
        "( sleep 3; leon notify --state working; leon send -b \"$LEON_BLOCK\" self; \
         leon send-key -b \"$LEON_BLOCK\" enter; touch '{}' ) &\n{READER}",
        done.display()
    );
    let w = o.env.ok(&["run", "--label", "worker", "--", "sh", "-c", &worker]);
    o.env.screen_until(&w, "the worker's prompt", |t| t.contains("ready>"));
    o.env.ok(&["notify", "-b", &w, "--state", "blocked", "--kind", "permission", "--body", "Bash: rm -rf build"]);
    until("the agent's own job", 20, || done.exists().then_some(()));
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(o.count(&w, "GOT: self"), 0, "BYPASS: the agent answered its own prompt:\n{}", o.env.capture(&w));
}

#[test]
fn another_pane_cannot_rewrite_the_prompt_or_run_config_actions() {
    let o = Orch::new();
    let w = held_worker(&o);
    let out = o.env.dir.path().join("rewrite");
    let body = format!(
        "leon notify -b {w} --state blocked --kind permission --body 'Bash: ls' 2>> '{o}.err'; echo $? >> '{o}.codes'\n\
         leon api call client.action '{{\"name\":\"x\"}}' 2>> '{o}.err'; echo $? >> '{o}.codes'\n\
         leon api call server.config_reload 2>> '{o}.err'; echo $? >> '{o}.codes'\ntouch '{o}'",
        o = out.display()
    );
    agent_beside(&o, &w, "rewriting-agent", &body, &out);
    let msg = o.env.json(&["block", "call", "-b", &w, "program_status"])["records"][0]["record"]["msg"].clone();
    assert_eq!(msg, "Bash: rm -rf build", "SPOOF: another pane rewrote what the person approves");
    let err = std::fs::read_to_string(format!("{}.err", out.display())).unwrap();
    assert_eq!(err.matches("permission prompt").count(), 3, "each refused for the prompt:\n{err}");
}
