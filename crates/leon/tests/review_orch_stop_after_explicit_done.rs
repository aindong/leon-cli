//! Review (correctness): a worker that reports its result with `leon notify --state done --body`
//! and then ends its turn makes one report, not a second one from its Stop hook.

mod support;

use support::orch::Orch;

const WORKER: &str = r#"leon notify --state done --body "Result: fixed"
printf '%s' '{"hook_event_name":"Stop","last_assistant_message":"Fixed it."}' | leon skill hook --agent claude --on stop
echo hooks-done
exec sleep 300"#;

#[test]
fn a_stop_hook_after_an_explicit_done_adds_no_second_report() {
    let o = Orch::new();
    let (s, boss) = o.env.session();
    let agent = o.script("claude-ish", WORKER);
    let w = o.ok_inside(&s, &boss, &["agent", "start", agent.to_str().unwrap(), "t"]);
    o.env.screen_until(&w, "the worker's hooks", |t| t.contains("hooks-done"));
    let inbox = o.json_inside(&s, &boss, &["inbox", "--json", "--peek"]);
    assert_eq!(inbox.as_array().unwrap().len(), 1, "one report per turn: {inbox}");
    assert_eq!(inbox[0]["body"], "Result: fixed", "the explicit result stays");
}
