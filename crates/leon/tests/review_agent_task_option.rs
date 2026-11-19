//! Review (security): a task that starts with `-` (from a file or stdin) never reaches the agent
//! as one of its options.

mod support;

use support::orch::Orch;

#[test]
fn a_task_that_looks_like_an_option_never_reaches_the_agent_as_one() {
    let o = Orch::new();
    let (s, boss) = o.env.session();
    let agent = o.script(
        "argv-agent",
        r#"for a in "$@"; do printf 'ARG<%s>\n' "$a" | head -n 1; done; echo argv-done; exec sleep 300"#,
    );
    let task = o.env.dir.path().join("task.md");
    std::fs::write(&task, "--allowedTools=Bash").unwrap();
    let w = o.ok_inside(&s, &boss, &["agent", "start", agent.to_str().unwrap(), "-f", task.to_str().unwrap()]);
    let screen = o.env.screen_until(&w, "the argv", |t| t.contains("argv-done"));
    assert!(
        !screen.contains("ARG<--allowedTools=Bash"),
        "INJECTION: the task reached the agent as an option:\n{screen}"
    );
}
