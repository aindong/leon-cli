//! The status hooks (`leon skill hook --agent A --on EVENT`): an agent's events set its block's
//! status, so the app shows it and an orchestrator hears of it. Events and fields, as documented
//! for Claude Code 2.1.311 and Codex 0.164: `UserPromptSubmit`; `PermissionRequest` (`tool_name`,
//! `tool_input`); Claude Code's `PreToolUse` for `AskUserQuestion` (`tool_input.questions`);
//! `PostToolUse` and Claude Code's `PostToolUseFailure`; `Stop` (`last_assistant_message`); Codex's
//! `Interrupt`. Like the session-start hook it never fails or stalls the agent, prints nothing (Codex
//! wants JSON or nothing from a Stop hook), and does nothing outside leon or for an agent that
//! isn't the one in its terminal's foreground.

use serde_json::{json, Value};

use super::resume::{interactive, own_block, print_mode, stdin_json};
use super::Agent;

/// A hook's input: a Stop's last message can be long; past this it is read as no message.
const MAX_INPUT: u64 = 1 << 20;

/// The agent event that ran the hook.
#[derive(Clone, Copy, PartialEq, Eq, Debug, clap::ValueEnum)]
pub enum On {
    UserPromptSubmit,
    PermissionRequest,
    PreToolUse,
    PostToolUse,
    PostToolUseFailure,
    Stop,
    Interrupt,
}

/// A status record: (state, kind, title, msg).
type Status = (&'static str, Option<&'static str>, Option<String>, Option<String>);

/// The status record for `on` with `input`; None for an event that changes nothing (a PreToolUse
/// for another tool).
fn status(on: On, input: &Value) -> Option<Status> {
    let text = |v: &Value| v.as_str().and_then(|s| one_line(s, 2048));
    Some(match on {
        // a subagent's tool says nothing about the main agent, which may still wait on a prompt
        On::PostToolUse | On::PostToolUseFailure if input.get("agent_id").is_some() => return None,
        On::UserPromptSubmit | On::PostToolUse | On::PostToolUseFailure => ("working", None, None, None),
        On::Interrupt => ("idle", None, None, None),
        On::Stop => ("done", None, None, text(&input["last_assistant_message"])),
        On::PreToolUse if input["tool_name"] == "AskUserQuestion" => question(input),
        On::PreToolUse => return None,
        On::PermissionRequest if input["tool_name"] == "AskUserQuestion" => question(input),
        On::PermissionRequest => {
            let tool = input["tool_name"].as_str().and_then(|t| one_line(t, 64));
            let i = &input["tool_input"];
            let what = match &i["command"] {
                Value::Array(argv) => {
                    one_line(&argv.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(" "), 2048)
                }
                c => text(c),
            };
            let what = what.or_else(|| text(&i["file_path"])).or_else(|| text(&i["description"]));
            let title = Some(tool.map_or_else(|| "Permission".to_string(), |t| format!("Permission: {t}")));
            ("blocked", Some("permission"), title, what)
        }
    })
}

fn question(input: &Value) -> Status {
    let q = input["tool_input"]["questions"][0]["question"].as_str().and_then(|s| one_line(s, 2048));
    ("blocked", Some("question"), Some("Question".into()), q)
}

/// `s` as one line of visible text that fits a status record: line breaks and controls become
/// spaces, runs of spaces one, invisible characters go, and it is cut to `max` bytes. None if empty.
fn one_line(s: &str, max: usize) -> Option<String> {
    let flat: String = s.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    let visible = leon_core::program_status::strip_hidden(&flat);
    let mut out = visible.split_whitespace().collect::<Vec<_>>().join(" ");
    if out.len() > max {
        let mut end = max - '…'.len_utf8();
        while !out.is_char_boundary(end) {
            end -= 1;
        }
        out.truncate(end);
        out.push('…');
    }
    (!out.is_empty()).then_some(out)
}

pub fn report(agent: Agent, on: On) {
    if std::env::var_os("LEON_BLOCK").is_none() {
        return;
    }
    let needs_input = !matches!(on, On::UserPromptSubmit | On::Interrupt);
    let input = if needs_input { stdin_json(MAX_INPUT).unwrap_or(Value::Null) } else { Value::Null };
    let Some((state, kind, title, msg)) = status(on, &input) else { return };
    if !interactive() || (agent == Agent::Claude && print_mode()) {
        return;
    }
    let Some((mut c, session, block)) = own_block() else { return };
    let method = |m: &str, args: Value| json!({ "session_id": session, "block_id": block, "method": m, "args": args });
    if on == On::Stop {
        // the agent reported its result itself this turn (`leon notify --state done --body`): keep it
        let now = c.call("block.method", method("dev.leon.terminal.program_status", json!({})));
        let root = now.ok().and_then(|r| r["records"].as_array()?.iter().find(|x| x["id"].is_null()).cloned());
        if root.is_some_and(|r| r["record"]["state"] == "done" && r["record"]["msg"].is_string()) {
            return;
        }
    }
    let args = json!({ "state": state, "kind": kind, "title": title, "msg": msg });
    let _ = c.call("block.method", method("dev.leon.terminal.report_status", args));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payloads_to_states() {
        let perm = json!({ "tool_name": "Bash", "tool_input": { "command": "rm -rf build\nls", "description": "x" } });
        let (s, k, t, m) = status(On::PermissionRequest, &perm).unwrap();
        assert_eq!(
            (s, k, t.as_deref(), m.as_deref()),
            ("blocked", Some("permission"), Some("Permission: Bash"), Some("rm -rf build ls"))
        );
        let codex = json!({ "tool_name": "shell", "tool_input": { "command": ["cargo", "publish"] } });
        assert_eq!(status(On::PermissionRequest, &codex).unwrap().3.as_deref(), Some("cargo publish"));
        let edit = json!({ "tool_name": "Edit", "tool_input": { "file_path": "/x/y.rs" } });
        assert_eq!(status(On::PermissionRequest, &edit).unwrap().3.as_deref(), Some("/x/y.rs"));
        assert_eq!(status(On::PermissionRequest, &Value::Null).unwrap().0, "blocked", "no payload still blocks");
        let ask = json!({ "tool_name": "AskUserQuestion", "tool_input": { "questions": [{ "question": "A or B?" }] } });
        assert_eq!(status(On::PreToolUse, &ask).unwrap().1, Some("question"));
        assert_eq!(status(On::PreToolUse, &ask).unwrap().3.as_deref(), Some("A or B?"));
        assert!(status(On::PreToolUse, &json!({ "tool_name": "Bash" })).is_none());
        let stop = json!({ "last_assistant_message": "Done.\n\nAll green." });
        assert_eq!(status(On::Stop, &stop).unwrap(), ("done", None, None, Some("Done. All green.".into())));
        assert_eq!(status(On::Stop, &json!({ "last_assistant_message": null })).unwrap().3, None);
        assert_eq!(status(On::Interrupt, &Value::Null).unwrap().0, "idle");
        assert_eq!(status(On::UserPromptSubmit, &Value::Null).unwrap().0, "working");
        assert_eq!(status(On::PostToolUse, &Value::Null).unwrap().0, "working");
        assert!(status(On::PostToolUse, &json!({ "agent_id": "a1" })).is_none(), "a subagent's tool");
    }

    #[test]
    fn one_line_cuts_on_a_char_boundary_and_drops_the_invisible() {
        let long = "é".repeat(2000);
        let cut = one_line(&long, 2048).unwrap();
        assert!(cut.len() <= 2048 && cut.ends_with('…'), "{}", cut.len());
        assert_eq!(one_line("a\u{202e}b\u{7}c\t\r\n d", 100).as_deref(), Some("ab c d"));
        assert_eq!(one_line(" \n\t", 100), None);
    }
}
