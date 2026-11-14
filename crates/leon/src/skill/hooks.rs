//! Our SessionStart hook inside a Claude Code `settings.json` or a Codex `hooks.json`. Both use
//! `{"hooks":{"SessionStart":[{"matcher"?,"hooks":[{"type":"command","command":…}]}]}}`.

use std::path::Path;

use serde_json::{json, Value};

use super::State;

/// Runs `leon skill hook` when leon is on PATH, and never fails the agent's session start.
pub const COMMAND: &str = "command -v leon >/dev/null 2>&1 && leon skill hook || true";

/// A hook entry is ours when its command runs `leon skill hook` (any version of our command).
fn is_ours(hook: &Value) -> bool {
    hook["command"].as_str().is_some_and(|c| c.contains("leon skill hook"))
}

/// The group setup adds: no matcher, so it runs on startup, resume, clear and compact.
fn our_group() -> Value {
    json!({ "hooks": [{ "type": "command", "command": COMMAND }] })
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

/// Current means exactly one entry of ours, in exactly the group setup writes.
pub fn state(v: &Value) -> State {
    match ours_count(v) {
        0 => State::Missing,
        1 if groups(v).contains(&our_group()) => State::Current,
        _ => State::Outdated,
    }
}

/// `v` without our entries. Groups, `SessionStart` and `hooks` that only held ours go too; anything
/// else (an empty group the user wrote, other keys) stays as it was.
pub fn without_ours(mut v: Value) -> Value {
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
    if starts.is_empty() {
        hooks.shift_remove("SessionStart");
    }
    if hooks.is_empty() {
        root.shift_remove("hooks");
    }
    v
}

/// `v` (or a new file) with exactly our group, appended after the user's own.
pub fn with_ours(v: Option<Value>) -> Value {
    let mut v = v.map_or_else(|| json!({}), without_ours);
    let root = v.as_object_mut().expect("read() checked");
    let hooks = root.entry("hooks").or_insert_with(|| json!({}));
    let starts = hooks.as_object_mut().expect("read() checked").entry("SessionStart").or_insert_with(|| json!([]));
    starts.as_array_mut().expect("read() checked").push(our_group());
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_then_remove_round_trips() {
        let mine = json!({ "a": 1, "hooks": { "Stop": [], "SessionStart": [{ "matcher": "x", "hooks": [] }] } });
        let added = with_ours(Some(mine.clone()));
        assert_eq!(state(&added), State::Current);
        assert_eq!(without_ours(added), mine, "an empty group the user wrote stays");
        assert_eq!(without_ours(with_ours(None)), json!({}));
    }

    #[test]
    fn duplicates_and_old_commands_are_outdated_and_replaced() {
        let old = json!({ "type": "command", "command": "leon skill hook" });
        let v = json!({ "hooks": { "SessionStart": [{ "hooks": [old, { "type": "command", "command": "x" }] }] } });
        assert_eq!(state(&v), State::Outdated);
        let fixed = with_ours(Some(v));
        assert_eq!(state(&fixed), State::Current);
        assert_eq!(fixed["hooks"]["SessionStart"][0]["hooks"], json!([{ "type": "command", "command": "x" }]));
        assert_eq!(state(&with_ours(Some(fixed.clone()))), State::Current);
        assert_eq!(state(&json!({ "hooks": { "SessionStart": [our_group(), our_group()] } })), State::Outdated);
    }
}
