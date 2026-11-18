//! Our hooks inside a Claude Code `settings.json` or a Codex `hooks.json`. Both use
//! `{"hooks":{"<Event>":[{"matcher"?,"hooks":[{"type":"command","command":…}]}]}}`. Each agent gets a
//! SessionStart hook (where it is, how to resume it) and one per event that sets its status.

use std::path::Path;

use serde_json::{json, Value};

use super::{Agent, State};

/// The events leon hooks for `agent`: (event, matcher, the `--on` its command passes). Claude Code
/// asks its questions with the AskUserQuestion tool; Codex has no question tool, but an
/// Interrupt event.
fn events(agent: Agent) -> &'static [(&'static str, Option<&'static str>, Option<&'static str>)] {
    match agent {
        Agent::Claude => &[
            ("SessionStart", None, None),
            ("UserPromptSubmit", None, Some("user-prompt-submit")),
            ("PermissionRequest", None, Some("permission-request")),
            ("PreToolUse", Some("AskUserQuestion"), Some("pre-tool-use")),
            ("PostToolUse", None, Some("post-tool-use")),
            ("PostToolUseFailure", None, Some("post-tool-use-failure")),
            ("Stop", None, Some("stop")),
        ],
        Agent::Codex => &[
            ("SessionStart", None, None),
            ("UserPromptSubmit", None, Some("user-prompt-submit")),
            ("PermissionRequest", None, Some("permission-request")),
            ("PostToolUse", None, Some("post-tool-use")),
            ("Stop", None, Some("stop")),
            ("Interrupt", None, Some("interrupt")),
        ],
    }
}

/// Every event either agent's hooks use: where to look for ours.
const ALL_EVENTS: &[&str] = &[
    "SessionStart",
    "UserPromptSubmit",
    "PermissionRequest",
    "PreToolUse",
    "PostToolUse",
    "PostToolUseFailure",
    "Stop",
    "Interrupt",
];

/// Runs `leon skill hook --agent <agent> [--on <event>]` when leon is on PATH, and never fails the
/// agent. The agent's name says which resume command the session-start hook records.
pub fn command(agent: Agent, on: Option<&str>) -> String {
    let on = on.map(|e| format!(" --on {e}")).unwrap_or_default();
    format!("command -v leon >/dev/null 2>&1 && leon skill hook --agent {}{on} || true", agent.name())
}

/// Commands an older leon wrote, so setup can replace them and remove can find them. A command is
/// ours only when it is one of these, or `command` for an agent and event, exactly: a user's hook
/// that merely mentions `leon skill hook` is theirs.
const OLD_COMMANDS: &[&str] = &["leon skill hook", "command -v leon >/dev/null 2>&1 && leon skill hook || true"];

fn is_ours(hook: &Value) -> bool {
    hook["command"].as_str().is_some_and(|c| {
        let c = c.trim();
        OLD_COMMANDS.contains(&c)
            || Agent::ALL.iter().any(|a| events(*a).iter().any(|(_, _, on)| command(*a, *on) == c))
    })
}

/// The group setup adds for an event: SessionStart's has no matcher, so it runs on startup, resume,
/// clear and compact.
fn our_group(agent: Agent, matcher: Option<&str>, on: Option<&str>) -> Value {
    let mut g = json!({ "hooks": [{ "type": "command", "command": command(agent, on) }] });
    if let Some(m) = matcher {
        g.as_object_mut().expect("an object").shift_insert(0, "matcher".into(), json!(m));
    }
    g
}

/// The file's JSON (`None` if it doesn't exist), or why we won't edit it.
pub fn read(path: &Path) -> Result<Option<Value>, String> {
    let text = match std::fs::read_to_string(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("{}: {e}", path.display())),
        Ok(t) => t,
    };
    let v: Value = serde_json::from_str(&text).map_err(|e| format!("{}: not JSON: {e}", path.display()))?;
    let bad = |what: String| Err(format!("{}: {what}", path.display()));
    if !v.is_object() {
        return bad("not a JSON object".into());
    }
    match v.get("hooks") {
        None => {}
        Some(Value::Object(h)) => {
            for event in ALL_EVENTS {
                match h.get(*event) {
                    None => {}
                    Some(Value::Array(groups)) => {
                        for g in groups {
                            if !g.is_object() || g.get("hooks").is_some_and(|h| !h.is_array()) {
                                return bad(format!("a hooks.{event} entry is not {{\"hooks\": [...]}}"));
                            }
                        }
                    }
                    Some(_) => return bad(format!("hooks.{event} is not an array")),
                }
            }
        }
        Some(_) => return bad("\"hooks\" is not an object".into()),
    }
    Ok(Some(v))
}

fn groups<'a>(v: &'a Value, event: &str) -> &'a [Value] {
    v["hooks"][event].as_array().map_or(&[], Vec::as_slice)
}

fn ours_count(v: &Value) -> usize {
    ALL_EVENTS
        .iter()
        .flat_map(|e| groups(v, e))
        .filter_map(|g| g["hooks"].as_array())
        .flatten()
        .filter(|h| is_ours(h))
        .count()
}

/// Current means exactly one entry of ours per event `agent` uses, each in exactly the group setup
/// writes.
pub fn state(v: &Value, agent: Agent) -> State {
    let all = events(agent);
    match ours_count(v) {
        0 => State::Missing,
        n if n == all.len() && all.iter().all(|(e, m, on)| groups(v, e).contains(&our_group(agent, *m, *on))) => {
            State::Current
        }
        _ => State::Outdated,
    }
}

/// `v` without our entries. Groups, events and `hooks` that only held ours go too; anything else
/// (an empty group the user wrote, other keys) stays as it was.
// shortcut: an event list or `hooks` the user had left empty before setup goes too; record what
// setup created if that ever matters.
pub fn without_ours(v: Value) -> Value {
    strip(v, true)
}

/// Take out our entries and the groups that held only them; with `prune`, also an emptied event
/// list and `hooks`.
fn strip(mut v: Value, prune: bool) -> Value {
    if ours_count(&v) == 0 {
        return v;
    }
    let root = v.as_object_mut().expect("read() checked");
    let hooks = root.get_mut("hooks").and_then(Value::as_object_mut).expect("ours_count found some");
    for event in ALL_EVENTS {
        let Some(list) = hooks.get_mut(*event).and_then(Value::as_array_mut) else { continue };
        if !list.iter().filter_map(|g| g["hooks"].as_array()).flatten().any(is_ours) {
            continue;
        }
        list.retain_mut(|g| match g.get_mut("hooks").and_then(Value::as_array_mut) {
            Some(l) if l.iter().any(is_ours) => {
                l.retain(|h| !is_ours(h));
                !l.is_empty()
            }
            _ => true,
        });
        if prune && list.is_empty() {
            hooks.shift_remove(*event);
        }
    }
    if prune && hooks.is_empty() {
        root.shift_remove("hooks");
    }
    v
}

/// `v` (or a new file) with exactly our groups, each appended after the user's own. Keys stay where
/// they are.
pub fn with_ours(v: Option<Value>, agent: Agent) -> Value {
    let mut v = v.map_or_else(|| json!({}), |v| strip(v, false));
    let root = v.as_object_mut().expect("read() checked");
    let hooks = root.entry("hooks").or_insert_with(|| json!({})).as_object_mut().expect("read() checked");
    for (event, matcher, on) in events(agent) {
        let list = hooks.entry(*event).or_insert_with(|| json!([]));
        list.as_array_mut().expect("read() checked").push(our_group(agent, *matcher, *on));
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_then_remove_round_trips() {
        let mine =
            json!({ "a": 1, "hooks": { "Notification": [], "SessionStart": [{ "matcher": "x", "hooks": [] }] } });
        let added = with_ours(Some(mine.clone()), Agent::Claude);
        assert_eq!(state(&added, Agent::Claude), State::Current);
        assert_eq!(without_ours(added), mine, "an empty group and an event leon doesn't use stay");
        let theirs = json!({ "hooks": { "Stop": [{ "hooks": [{ "type": "command", "command": "say hi" }] }] } });
        assert_eq!(
            without_ours(with_ours(Some(theirs.clone()), Agent::Codex)),
            theirs,
            "a user's hook on our event stays"
        );
        assert_eq!(without_ours(with_ours(None, Agent::Codex)), json!({}));
    }

    #[test]
    fn duplicates_and_old_commands_are_outdated_and_replaced() {
        let old = json!({ "type": "command", "command": "leon skill hook" });
        let v = json!({ "hooks": { "SessionStart": [{ "hooks": [old, { "type": "command", "command": "x" }] }] } });
        assert_eq!(state(&v, Agent::Claude), State::Outdated);
        let fixed = with_ours(Some(v), Agent::Claude);
        assert_eq!(state(&fixed, Agent::Claude), State::Current);
        assert_eq!(fixed["hooks"]["SessionStart"][0]["hooks"], json!([{ "type": "command", "command": "x" }]));
        assert_eq!(state(&with_ours(Some(fixed.clone()), Agent::Claude), Agent::Claude), State::Current);
        assert_eq!(state(&fixed, Agent::Codex), State::Outdated, "the other agent's command is outdated here");
        let twice = json!({ "hooks": { "SessionStart": [our_group(Agent::Codex, None, None), our_group(Agent::Codex, None, None)] } });
        assert_eq!(state(&twice, Agent::Codex), State::Outdated);
    }

    #[test]
    fn ours_means_exactly_our_command() {
        let cmd = |c: &str| json!({ "command": c });
        assert!(is_ours(&cmd(&command(Agent::Claude, None))));
        assert!(is_ours(&cmd(&command(Agent::Codex, None))));
        assert!(is_ours(&cmd(&command(Agent::Claude, Some("stop")))));
        assert!(!is_ours(&cmd(&command(Agent::Claude, Some("made-up")))), "only events leon hooks");
        assert!(is_ours(&cmd(OLD_COMMANDS[1])));
        assert!(is_ours(&cmd("leon skill hook")));
        assert!(!is_ours(&cmd("~/bin/ctx.sh; leon skill hook --legacy")));
        assert!(!is_ours(&cmd("napoleon skill hook")));
    }

    #[test]
    fn replacing_an_old_entry_keeps_key_order() {
        let old = json!({ "hooks": { "SessionStart": [{ "hooks": [{ "command": "leon skill hook" }] }] }, "z": 1 });
        let new = with_ours(Some(old), Agent::Codex);
        assert_eq!(new.as_object().unwrap().keys().collect::<Vec<_>>(), ["hooks", "z"]);
        assert_eq!(state(&new, Agent::Codex), State::Current);
    }
}
