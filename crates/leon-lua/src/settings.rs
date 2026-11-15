//! `leon.set`: the config's settings.

use mlua::{Lua, Value};

use crate::api::{state, Bad};

/// `leon.set(name, value)`: a setting: `resume` (`"ask"`, `"always"`, `"never"`), `repeat_time`
/// (milliseconds), `hint_patterns` (a list of regular expressions), or one of the booleans
/// `copy_on_select`, `notify_blocked`, `notify_error`, `notify_done`.
pub(crate) fn set(lua: &Lua, (name, value): (String, Value)) -> Result<(), Bad> {
    if name == "repeat_time" {
        let ms = match value {
            Value::Integer(i) => Some(i as f64),
            Value::Number(n) => Some(n),
            _ => None,
        };
        let ms = ms.filter(|ms| ms.fract() == 0. && (0. ..=10_000.).contains(ms));
        let ms = ms.ok_or_else(|| Bad("repeat_time is whole milliseconds, 0 to 10000 (0 turns repeat off)".into()))?;
        state(lua).cfg.repeat_time = Some(ms as u64);
        return Ok(());
    }
    if name == "hint_patterns" {
        state(lua).cfg.hint_patterns = hint_patterns(value)?;
        return Ok(());
    }
    let mut st = state(lua);
    let cfg = &mut st.cfg;
    let flag = match name.as_str() {
        "copy_on_select" => Some(&mut cfg.copy_on_select),
        "notify_blocked" => Some(&mut cfg.notify_blocked),
        "notify_error" => Some(&mut cfg.notify_error),
        "notify_done" => Some(&mut cfg.notify_done),
        _ => None,
    };
    if let Some(flag) = flag {
        let Value::Boolean(on) = value else { return Err(format!("{name} must be true or false").into()) };
        *flag = Some(on);
        return Ok(());
    }
    drop(st);
    if name != "resume" {
        return Err(format!(
            "unknown setting {name:?}; the settings are \"resume\", \"copy_on_select\", \"repeat_time\", \
             \"hint_patterns\", \"notify_blocked\", \"notify_error\" and \"notify_done\""
        )
        .into());
    }
    let text = match &value {
        Value::String(s) => s.to_str().map(|s| s.to_string()).unwrap_or_default(),
        _ => String::new(),
    };
    let policy = crate::Resume::ALL.into_iter().find(|r| r.as_str() == text);
    let policy = policy.ok_or_else(|| Bad("resume must be \"ask\", \"always\" or \"never\"".into()))?;
    state(lua).cfg.resume = Some(policy);
    Ok(())
}

/// The config's hint patterns: a list of at most `MAX_PATTERNS` strings, each a regular expression
/// leon_core::hints compiles.
fn hint_patterns(value: Value) -> Result<Vec<String>, Bad> {
    use leon_core::hints::{compile, MAX_PATTERNS};
    let Value::Table(t) = value else {
        return Err(format!("hint_patterns must be a list of regular expressions, got {}", value.type_name()).into());
    };
    let len = t.raw_len();
    if t.pairs::<Value, Value>().count() != len {
        return Err("hint_patterns must be a list of regular expressions, not a table with keys".into());
    }
    if len > MAX_PATTERNS {
        return Err(format!("at most {MAX_PATTERNS} hint patterns, got {len}").into());
    }
    let mut out = Vec::new();
    for v in t.sequence_values::<Value>() {
        let Value::String(s) = v? else { return Err("hint_patterns holds strings only".into()) };
        let p = s.to_str().map_err(|e| Bad(e.to_string()))?.to_string();
        compile(&p).map_err(Bad)?;
        out.push(p);
    }
    Ok(out)
}
