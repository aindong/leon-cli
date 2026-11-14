//! Our SessionStart hook inside a Claude Code `settings.json` or a Codex `hooks.json`. Both use
//! `{"hooks":{"SessionStart":[{"matcher"?,"hooks":[{"type":"command","command":…}]}]}}`.

use std::path::Path;

use serde_json::{json, Value};

use super::{Agent, State};

/// Runs `leon skill hook --agent <agent>` when leon is on PATH, and never fails the agent's
/// session start. The agent's name says which resume command the hook records.
pub fn command(agent: Agent) -> String {
    format!("command -v leon >/dev/null 2>&1 && leon skill hook --agent {} || true", agent.name())
}

/// Commands an older leon wrote, so setup can replace them and remove can find them. A command is
/// ours only when it is one of these, or `command` for an agent, exactly: a user's hook that
/// merely mentions `leon skill hook` is theirs.
const OLD_COMMANDS: &[&str] = &["leon skill hook", "command -v leon >/dev/null 2>&1 && leon skill hook || true"];

fn is_ours(hook: &Value) -> bool {
    hook["command"].as_str().is_some_and(|c| {
        let c = c.trim();
        OLD_COMMANDS.contains(&c) || Agent::ALL.iter().any(|a| command(*a) == c)
    })
}

/// The group setup adds: no matcher, so it runs on startup, resume, clear and compact.
fn our_group(agent: Agent) -> Value {
    json!({ "hooks": [{ "type": "command", "command": command(agent) }] })
}

/// The file's JSON (`None` if it doesn't exist), or why we won't edit it.
pub fn read(path: &Path) -> Result<Option<Value>, String> {
    let text = match std::fs::read_to_string(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("{}: {e}", path.display())),
        Ok(t) => t,
    };
    let v: Value = serde_json::from_str(&text).map_err(|e| format!("{}: not JSON: {e}", path.display()))?;
    let bad = |what: &str| Err(format!("{}: {what}", path.display()));
    if !v.is_object() {
        return bad("not a JSON object");
    }
    match v.get("hooks") {
        None => {}
        Some(Value::Object(h)) => match h.get("SessionStart") {
            None => {}
            Some(Value::Array(groups)) => {
                for g in groups {
                    if !g.is_object() || g.get("hooks").is_some_and(|h| !h.is_array()) {
                        return bad("a hooks.SessionStart entry is not {\"hooks\": [...]}");
                    }
                }
            }
            Some(_) => return bad("hooks.SessionStart is not an array"),
        },
        Some(_) => return bad("\"hooks\" is not an object"),
    }
    Ok(Some(v))
}

fn groups(v: &Value) -> &[Value] {
    v["hooks"]["SessionStart"].as_array().map_or(&[], Vec::as_slice)
}

fn ours_count(v: &Value) -> usize {
    groups(v).iter().filter_map(|g| g["hooks"].as_array()).flatten().filter(|h| is_ours(h)).count()
}

/// Current means exactly one entry of ours, in exactly the group setup writes for `agent`.
pub fn state(v: &Value, agent: Agent) -> State {
    match ours_count(v) {
        0 => State::Missing,
        1 if groups(v).contains(&our_group(agent)) => State::Current,
        _ => State::Outdated,
    }
}

/// `v` without our entries. Groups, `SessionStart` and `hooks` that only held ours go too; anything
/// else (an empty group the user wrote, other keys) stays as it was.
// shortcut: a `hooks` or `SessionStart` the user had left empty before setup goes too; record what
// setup created if that ever matters.
pub fn without_ours(v: Value) -> Value {
    strip(v, true)
}

/// Take out our entries and the groups that held only them; with `prune`, also an emptied
/// `SessionStart` and `hooks`.
fn strip(mut v: Value, prune: bool) -> Value {
    if ours_count(&v) == 0 {
        return v;
    }
    let root = v.as_object_mut().expect("read() checked");
    let hooks = root.get_mut("hooks").and_then(Value::as_object_mut).expect("ours_count found some");
    let starts = hooks.get_mut("SessionStart").and_then(Value::as_array_mut).expect("ours_count found some");
    starts.retain_mut(|g| match g.get_mut("hooks").and_then(Value::as_array_mut) {
        Some(list) if list.iter().any(is_ours) => {
            list.retain(|h| !is_ours(h));
            !list.is_empty()
        }
        _ => true,
    });
    if prune && starts.is_empty() {
        hooks.shift_remove("SessionStart");
    }
    if prune && hooks.is_empty() {
        root.shift_remove("hooks");
    }
    v
}

/// `v` (or a new file) with exactly our group, appended after the user's own. Keys stay where they are.
pub fn with_ours(v: Option<Value>, agent: Agent) -> Value {
    let mut v = v.map_or_else(|| json!({}), |v| strip(v, false));
    let root = v.as_object_mut().expect("read() checked");
    let hooks = root.entry("hooks").or_insert_with(|| json!({}));
    let starts = hooks.as_object_mut().expect("read() checked").entry("SessionStart").or_insert_with(|| json!([]));
    starts.as_array_mut().expect("read() checked").push(our_group(agent));
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_then_remove_round_trips() {
        let mine = json!({ "a": 1, "hooks": { "Stop": [], "SessionStart": [{ "matcher": "x", "hooks": [] }] } });
        let added = with_ours(Some(mine.clone()), Agent::Claude);
        assert_eq!(state(&added, Agent::Claude), State::Current);
        assert_eq!(without_ours(added), mine, "an empty group the user wrote stays");
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
        let twice = json!({ "hooks": { "SessionStart": [our_group(Agent::Codex), our_group(Agent::Codex)] } });
        assert_eq!(state(&twice, Agent::Codex), State::Outdated);
    }

    #[test]
    fn ours_means_exactly_our_command() {
        let cmd = |c: &str| json!({ "command": c });
        assert!(is_ours(&cmd(&command(Agent::Claude))));
        assert!(is_ours(&cmd(&command(Agent::Codex))));
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
