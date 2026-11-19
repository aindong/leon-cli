//! Agent orchestration criterion 2: a worker's hooks (fed the JSON Claude Code gives them) set its
//! state, and its parent's `leon inbox` lists what needs the parent: the permission prompt, the
//! question and the finished turn with its last message. Reading marks them read.

mod support;

use support::orch::{until, Orch};

const WORKER: &str = r#"hook() { printf '%s' "$2" | leon skill hook --agent claude --on "$1"; }
hook user-prompt-submit '{"hook_event_name":"UserPromptSubmit","prompt":"go"}'
hook permission-request '{"hook_event_name":"PermissionRequest","tool_name":"Bash","tool_input":{"command":"rm -rf build","description":"Clean"}}'
hook post-tool-use '{"hook_event_name":"PostToolUse","tool_name":"Bash"}'
hook pre-tool-use '{"hook_event_name":"PreToolUse","tool_name":"AskUserQuestion","tool_input":{"questions":[{"question":"Which database?"}]}}'
hook post-tool-use '{"hook_event_name":"PostToolUse","tool_name":"AskUserQuestion"}'
hook stop '{"hook_event_name":"Stop","last_assistant_message":"All tests pass.\nTwo files changed."}'
echo hooks-done
exec sleep 300"#;

#[test]
fn hook_reports_reach_the_parents_inbox_once() {
    let o = Orch::new();
    let (s, boss) = o.env.session();
    let agent = o.script("claude-ish", WORKER);
    let w = o.ok_inside(&s, &boss, &["agent", "start", "--label", "builder", agent.to_str().unwrap(), "build it"]);
    o.env.screen_until(&w, "the worker's hooks", |t| t.contains("hooks-done"));

    let team = o.json_inside(&s, &boss, &["agent", "ls", "--json"]);
    let me = &team["workers"][0];
    assert_eq!(me["unread"], 3, "three reports, none for working: {team}");
    assert_eq!(me["state"], "done", "the last state reported: {team}");

    let text = o.ok_inside(&s, &boss, &["inbox", "--peek"]);
    assert!(text.contains(&format!("leon capture -b {w}")), "says how to look: {text}");
    assert!(text.contains("builder"), "names the worker: {text}");

    let inbox = o.json_inside(&s, &boss, &["inbox", "--json"]);
    let r = inbox.as_array().unwrap_or_else(|| panic!("a list: {inbox}"));
    assert_eq!(r.len(), 3, "{inbox}");
    for x in r {
        assert_eq!(x["block_id"], w.as_str());
        assert_eq!(x["label"], "builder");
        assert_eq!(x["agent"], "claude-ish");
    }
    assert_eq!((r[0]["state"].as_str(), r[0]["kind"].as_str()), (Some("blocked"), Some("permission")));
    assert!(r[0]["body"].as_str().unwrap().contains("rm -rf build"), "what it asks: {}", r[0]);
    assert!(r[0]["title"].as_str().unwrap().contains("Bash"), "{}", r[0]);
    assert_eq!((r[1]["state"].as_str(), r[1]["kind"].as_str()), (Some("blocked"), Some("question")));
    assert_eq!(r[1]["body"], "Which database?");
    assert_eq!(r[2]["state"], "done");
    assert_eq!(r[2]["body"], "All tests pass. Two files changed.", "the last message on one line");

    assert_eq!(o.json_inside(&s, &boss, &["inbox", "--json"]), serde_json::json!([]), "read once");
    assert_eq!(o.json_inside(&s, &boss, &["agent", "ls", "--json"])["workers"][0]["unread"], 0);
}

#[test]
fn an_explicit_report_carries_its_body_and_the_hook_after_it_adds_nothing() {
    let o = Orch::new();
    let (s, boss) = o.env.session();
    let agent = o.script("quiet-agent", "exec sleep 300");
    let w = o.ok_inside(&s, &boss, &["agent", "start", agent.to_str().unwrap(), "t"]);
    o.ok_inside(&s, &w, &["notify", "--state", "blocked", "--kind", "question", "--body", "Postgres or SQLite?"]);
    o.ok_inside(&s, &w, &["notify", "--state", "done", "--body", "Result: 3 bugs fixed"]);
    o.ok_inside(&s, &w, &["notify", "--state", "done"]); // as a Stop hook with no message would
    let inbox = until("two reports", 10, || {
        let v = o.json_inside(&s, &boss, &["inbox", "--json", "--peek"]);
        (v.as_array().unwrap().len() >= 2).then_some(v)
    });
    let r = inbox.as_array().unwrap();
    assert_eq!(r.len(), 2, "the bodiless done after an unread done adds nothing: {inbox}");
    assert_eq!((r[0]["kind"].as_str(), r[0]["body"].as_str()), (Some("question"), Some("Postgres or SQLite?")));
    assert_eq!(r[1]["body"], "Result: 3 bugs fixed");
    let out = o.run_inside(&s, &w, &["notify", "--state", "done", "--kind", "question"]);
    assert_eq!(out.status.code(), Some(2), "--kind goes with --state blocked");
}
