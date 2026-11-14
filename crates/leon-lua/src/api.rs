//! The `leon` table's config functions, and the sandbox around every Lua state.

use std::path::Path;

use mlua::{FromLua, FromLuaMulti, Function, IntoLuaMulti, Lua, LuaSerdeExt, MultiValue, Table, Value};
use serde_json::json;

use crate::keys::{self, KeySpec};
use crate::{Action, Binding, Config, Diagnostic, Level, Mode, Target, Unbind};

/// The config (or script) declarations, kept in the Lua state's app data.
#[derive(Default)]
pub(crate) struct State {
    pub(crate) entries: Vec<Entry>,
    pub(crate) next_id: i64,
    /// Everything but `bindings`, which come from `entries` at the end.
    pub(crate) cfg: Config,
    /// `run` functions of `leon.action`, by name.
    pub(crate) runs: Vec<(String, Function)>,
    /// Past the load (or in a script): config-only calls refuse, bad calls aren't diagnosed, logs go to stderr.
    pub(crate) live: bool,
}

pub(crate) struct Entry {
    /// Lets `leon.unbind(binding)` find exactly one.
    id: i64,
    pub(crate) b: Binding,
    pub(crate) func: Option<Function>,
}

/// Why a `leon.*` call was skipped.
pub(crate) struct Bad(pub(crate) String);

impl From<String> for Bad {
    fn from(s: String) -> Self {
        Bad(s)
    }
}

impl From<&str> for Bad {
    fn from(s: &str) -> Self {
        Bad(s.into())
    }
}

impl From<mlua::Error> for Bad {
    fn from(e: mlua::Error) -> Self {
        Bad(e.to_string())
    }
}

/// Run a config chunk into `lua`, whose `State` app data is already set.
pub(crate) fn exec_chunk(lua: &Lua, path: &Path, src: &[u8]) -> mlua::Result<()> {
    // Like luaL_loadfile (which `require` uses), ignore a `#` first line but keep its newline.
    let src =
        if src.starts_with(b"#") { &src[src.iter().position(|&b| b == b'\n').unwrap_or(src.len())..] } else { src };
    lua.load(src).set_name(format!("@{}", path.display())).set_mode(mlua::chunk::ChunkMode::Text).exec()
}

/// The bindings so far, as a `Config` would list them.
pub(crate) fn config(lua: &Lua) -> Config {
    let st = state(lua);
    let c = &st.cfg;
    Config {
        bindings: st.entries.iter().map(|e| e.b.clone()).collect(),
        modes: c.modes.clone(),
        actions: c.actions.clone(),
        unbinds: c.unbinds.clone(),
        diagnostics: c.diagnostics.clone(),
        logs: c.logs.clone(),
        resume: c.resume,
        copy_on_select: c.copy_on_select,
        repeat_time: c.repeat_time,
        hint_patterns: c.hint_patterns.clone(),
    }
}

pub(crate) fn state(lua: &Lua) -> mlua::AppDataRefMut<'_, State> {
    lua.app_data_mut::<State>().expect("leon state")
}

/// `file:line` of the Lua code calling the running `leon.*` function.
/// Skips C frames, so `pcall(leon.bind, …)` still points at the config line.
pub(crate) fn caller(lua: &Lua) -> String {
    (1..)
        .map_while(|level| {
            lua.inspect_stack(level, |d| {
                let src = d.source().source?.strip_prefix('@')?.to_string();
                Some(format!("{src}:{}", d.current_line().unwrap_or(0)))
            })
        })
        .flatten()
        .next()
        .unwrap_or_else(|| "?".into())
}

/// A `leon.NAME` function that never raises: a bad call is recorded as an error diagnostic at the
/// caller's line and returns `nil, message`.
pub(crate) fn func<A, R>(
    lua: &Lua,
    name: &'static str,
    f: impl Fn(&Lua, A) -> Result<R, Bad> + 'static,
) -> mlua::Result<Function>
where
    A: FromLuaMulti,
    R: IntoLuaMulti,
{
    lua.create_function(move |lua, args: MultiValue| {
        match A::from_lua_multi(args, lua).map_err(Bad::from).and_then(|a| f(lua, a)) {
            Ok(r) => r.into_lua_multi(lua),
            Err(Bad(msg)) => {
                if !state(lua).live {
                    let at = caller(lua);
                    let message = format!("leon.{name}: {msg}");
                    state(lua).cfg.diagnostics.push(Diagnostic { level: Level::Error, at, message });
                }
                (Value::Nil, msg).into_lua_multi(lua)
            }
        }
    })
}

pub(crate) fn field<T: FromLua>(t: &Table, k: &str) -> Result<Option<T>, Bad> {
    t.get::<Option<T>>(k).map_err(|e| Bad(format!("{k}: {e}")))
}

/// A boolean option; Lua truthiness would turn `"false"` or `0` into true.
pub(crate) fn flag(t: &Table, k: &str) -> Result<bool, Bad> {
    match t.get::<Value>(k)? {
        Value::Nil => Ok(false),
        Value::Boolean(b) => Ok(b),
        v => Err(format!("{k} must be true or false, got {}", v.type_name()).into()),
    }
}

fn required<T: FromLua>(t: &Table, k: &str) -> Result<T, Bad> {
    field(t, k)?.ok_or_else(|| Bad(format!("{k} is required")))
}

/// A config-only call: refused once the config has loaded (from an action, say).
fn loading<A, R>(f: impl Fn(&Lua, A) -> Result<R, Bad> + 'static) -> impl Fn(&Lua, A) -> Result<R, Bad> + 'static {
    move |lua, a| {
        if state(lua).live {
            return Err("only works while the config loads".into());
        }
        f(lua, a)
    }
}

/// The `leon` table's declaration functions: every config one, or the few a script also has.
pub(crate) fn leon_table(lua: &Lua, script: bool) -> mlua::Result<Table> {
    let leon = lua.create_table()?;
    if script {
        leon.set("action", func(lua, "action", action)?)?;
    } else {
        leon.set("bind", func(lua, "bind", loading(bind))?)?;
        leon.set("unbind", func(lua, "unbind", loading(unbind))?)?;
        leon.set("mode", func(lua, "mode", loading(mode))?)?;
        leon.set("bindings", func(lua, "bindings", loading(bindings))?)?;
        leon.set("modes", func(lua, "modes", loading(modes))?)?;
        leon.set("action", func(lua, "action", loading(action))?)?;
        leon.set("on", func(lua, "on", on_in_config)?)?;
        leon.set("set", func(lua, "set", loading(set))?)?;
    }
    leon.set("log", func(lua, "log", log)?)?;
    let json = lua.create_table()?;
    json.set(
        "encode",
        func(lua, "json.encode", |_, (v, opts): (Value, Option<Table>)| {
            let indent = match opts {
                Some(t) => flag(&t, "indent")?,
                None => false,
            };
            let s = if indent { serde_json::to_string_pretty(&v) } else { serde_json::to_string(&v) };
            s.map_err(|e| Bad(e.to_string()))
        })?,
    )?;
    json.set(
        "decode",
        func(lua, "json.decode", |lua, text: String| {
            let v: serde_json::Value = serde_json::from_str(&text).map_err(|e| Bad(e.to_string()))?;
            Ok(lua.to_value(&v)?)
        })?,
    )?;
    leon.set("json", json)?;
    Ok(leon)
}

/// `leon.bind`'s options (the 4th argument): `repeats` (tmux `bind -r`; `["repeat"]` too, since
/// `repeat` is a Lua keyword).
fn bind_options(opts: Option<Value>) -> Result<bool, Bad> {
    let t = match opts {
        None => return Ok(false),
        Some(Value::Table(t)) => t,
        Some(v) => {
            return Err(format!("options must be a table like {{ repeats = true }}, got {}", v.type_name()).into())
        }
    };
    for pair in t.pairs::<Value, Value>() {
        let (k, _) = pair?;
        let name = k.to_string().unwrap_or_default();
        if name != "repeats" && name != "repeat" {
            return Err(format!("unknown option {name:?}: the option is repeats").into());
        }
    }
    Ok(flag(&t, "repeats")? || flag(&t, "repeat")?)
}

fn bind(lua: &Lua, (key, target, args, opts): (String, Value, Option<Value>, Option<Value>)) -> Result<(), Bad> {
    let key = keys::parse(&key)?;
    if let Some(Value::Table(t)) = &args {
        if t.contains_key("repeats")? || t.contains_key("repeat")? {
            return Err(
                "repeats is an option, the 4th argument: leon.bind(key, action, nil, { repeats = true })".into()
            );
        }
    }
    let repeats = bind_options(opts)?;
    let (target, func) = match target {
        Value::String(s) => {
            let name = s.to_string_lossy();
            if name.is_empty() {
                return Err("empty action name".into());
            }
            let args = match args {
                None => None,
                Some(v @ Value::Table(_)) => Some(serde_json::to_value(&v).map_err(|e| Bad(format!("args: {e}")))?),
                Some(v) => return Err(format!("args must be a table, got {}", v.type_name()).into()),
            };
            (Target::Action { name, args }, None)
        }
        Value::Function(f) => {
            if key.mode.is_some() {
                return Err(format!("{key}: mode bindings must name an action, not a function").into());
            }
            if args.is_some() {
                return Err("args go with an action name, not a function".into());
            }
            (Target::Function, Some(f))
        }
        v => return Err(format!("expected an action name or a function, got {}", v.type_name()).into()),
    };
    let source = caller(lua);
    let mut st = state(lua);
    if let Some(m) = &key.mode {
        if !st.cfg.modes.iter().any(|x| &x.name == m) {
            st.cfg.modes.push(Mode { name: m.clone(), exclusive: false, prefix: None, source: source.clone() });
        }
    }
    // A key is either an immediate binding or a prefix of longer ones: newest wins.
    let mut replaced = Vec::new();
    st.entries.retain(|e| {
        let old = &e.b.key;
        let n = old.seq.len().min(key.seq.len());
        let clash = old.mode == key.mode && old.seq.len() != key.seq.len() && old.seq[..n] == key.seq[..n];
        if clash {
            replaced.push(format!(
                "{key} replaces {old} ({}): a key is either a binding or a prefix, newest wins",
                e.b.source
            ));
        }
        !clash
    });
    for message in replaced {
        st.cfg.diagnostics.push(Diagnostic { level: Level::Warning, at: source.clone(), message });
    }
    st.next_id += 1;
    let id = st.next_id;
    st.entries.push(Entry { id, b: Binding { key, target, source, repeats }, func });
    Ok(())
}

fn unbind(lua: &Lua, (what, target): (Value, Option<Value>)) -> Result<usize, Bad> {
    let t = match what {
        Value::String(s) => return unbind_key(lua, keys::parse(&s.to_string_lossy())?, target),
        Value::Table(t) => t,
        v => return Err(format!("expected a key, a binding or {{mode = name}}, got {}", v.type_name()).into()),
    };
    if let Some(id) = field::<i64>(&t, "id")? {
        let mut st = state(lua);
        let before = st.entries.len();
        st.entries.retain(|e| e.id != id);
        return Ok(before - st.entries.len());
    }
    let Some(name) = field::<String>(&t, "mode")? else {
        return Err("expected a key, a binding from leon.bindings(), or {mode = name}".into());
    };
    // shortcut: removes the file's mode only; record it like `unbinds` once client defaults have modes.
    let mut st = state(lua);
    st.cfg.modes.retain(|m| m.name != name);
    let before = st.entries.len();
    st.entries.retain(|e| e.b.key.mode.as_deref() != Some(&name));
    Ok(before - st.entries.len())
}

fn unbind_key(lua: &Lua, key: KeySpec, target: Option<Value>) -> Result<usize, Bad> {
    let (action, func) = match target {
        None => (None, None),
        Some(Value::String(s)) => (Some(s.to_string_lossy()), None),
        Some(Value::Function(f)) => (None, Some(f.to_pointer())),
        Some(v) => return Err(format!("target must be an action name or a function, got {}", v.type_name()).into()),
    };
    let mut st = state(lua);
    let before = st.entries.len();
    st.entries.retain(|e| {
        let hit = match (&action, func, &e.b.target) {
            (Some(a), _, Target::Action { name, .. }) => a == name,
            (Some(_), _, Target::Function) => false,
            (None, Some(p), _) => e.func.as_ref().is_some_and(|f| f.to_pointer() == p),
            (None, None, _) => true,
        };
        !(hit && e.b.key == key)
    });
    let n = before - st.entries.len();
    if func.is_none() {
        // client defaults only bind actions, so a function target never reaches them
        st.cfg.unbinds.push(Unbind { key, action });
    }
    Ok(n)
}

fn mode(lua: &Lua, (name, opts): (String, Option<Table>)) -> Result<(), Bad> {
    if !keys::valid_mode(&name) {
        return Err(format!("bad mode name {name:?}: use [a-z0-9_-]").into());
    }
    let (exclusive, prefix) = match &opts {
        Some(t) => (flag(t, "exclusive")?, field::<String>(t, "prefix")?),
        None => (false, None),
    };
    let prefix = match prefix {
        Some(p) => match keys::parse(&p) {
            Ok(k) if k.mode.is_none() && k.seq.len() == 1 => Some(k.sequence()),
            Ok(_) => return Err(format!("prefix {p:?} must be one chord, like \"ctrl+a\"").into()),
            Err(e) => return Err(format!("prefix: {e}").into()),
        },
        None => None,
    };
    let source = caller(lua);
    let mut st = state(lua);
    match st.cfg.modes.iter_mut().find(|m| m.name == name) {
        Some(m) => (m.exclusive, m.prefix) = (exclusive, prefix),
        None => st.cfg.modes.push(Mode { name, exclusive, prefix, source }),
    }
    Ok(())
}

/// `leon.set(name, value)`: a setting: `resume` (`"ask"`, `"always"`, `"never"`),
/// `copy_on_select` (a boolean), `repeat_time` (milliseconds) or `hint_patterns` (a list of
/// regular expressions).
fn set(lua: &Lua, (name, value): (String, Value)) -> Result<(), Bad> {
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
    if name == "copy_on_select" {
        let Value::Boolean(on) = value else { return Err("copy_on_select must be true or false".into()) };
        state(lua).cfg.copy_on_select = Some(on);
        return Ok(());
    }
    if name != "resume" {
        return Err(format!(
            "unknown setting {name:?}; the settings are \"resume\", \"copy_on_select\", \"repeat_time\" and \"hint_patterns\""
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

fn bindings(lua: &Lua, filter: Option<Table>) -> Result<Table, Bad> {
    let (key, mode, action) = match &filter {
        Some(t) => (field::<String>(t, "key")?, field::<String>(t, "mode")?, field::<String>(t, "action")?),
        None => (None, None, None),
    };
    let key = key.map(|k| keys::parse(&k)).transpose()?;
    let rows: Vec<_> = state(lua)
        .entries
        .iter()
        .filter(|e| {
            let k = &e.b.key;
            key.as_ref().is_none_or(|f| f.seq == k.seq && (f.mode.is_none() || f.mode == k.mode))
                && mode.as_ref().is_none_or(|m| k.mode.as_ref() == Some(m))
                && action.as_ref().is_none_or(|a| matches!(&e.b.target, Target::Action { name, .. } if name == a))
        })
        .map(|e| (e.id, e.b.clone(), e.func.clone()))
        .collect();
    let out = lua.create_table()?;
    for (id, b, func) in rows {
        let t = lua.create_table()?;
        t.set("id", id)?;
        t.set("key", b.key.sequence())?;
        t.set("mode", b.key.mode)?;
        t.set("source", b.source)?;
        t.set("repeats", b.repeats)?;
        match b.target {
            Target::Action { name, args } => {
                t.set("action", name)?;
                if let Some(a) = args {
                    t.set("args", lua.to_value(&a)?)?;
                }
            }
            Target::Function => t.set("fn", func)?,
        }
        out.push(t)?;
    }
    Ok(out)
}

fn modes(lua: &Lua, (): ()) -> Result<Table, Bad> {
    let rows = state(lua).cfg.modes.clone();
    let out = lua.create_table()?;
    for m in rows {
        let t = lua.create_table()?;
        t.set("name", m.name)?;
        t.set("exclusive", m.exclusive)?;
        t.set("source", m.source)?;
        out.push(t)?;
    }
    Ok(out)
}

fn action(lua: &Lua, t: Table) -> Result<(), Bad> {
    let name: String = required(&t, "name")?;
    if name.is_empty() || !name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_') {
        return Err(format!("bad action name {name:?}: use [a-z0-9_], no dot").into());
    }
    let title: String = required(&t, "title")?;
    let run: Function = required(&t, "run")?;
    let args = field::<Table>(&t, "args")?.map(args_schema).transpose()?;
    let source = caller(lua);
    let a = Action {
        name,
        title,
        description: field(&t, "description")?,
        category: field(&t, "category")?.unwrap_or_else(|| "Actions".into()),
        keywords: field(&t, "keywords")?.unwrap_or_default(),
        args,
        repeats: flag(&t, "repeats")?,
        source,
    };
    let mut st = state(lua);
    if let Some(i) = st.cfg.actions.iter().position(|x| x.name == a.name) {
        let old = st.cfg.actions.remove(i);
        let message = format!("action {:?} redefined (was {}), newest wins", a.name, old.source);
        st.cfg.diagnostics.push(Diagnostic { level: Level::Warning, at: a.source.clone(), message });
    }
    st.runs.retain(|(n, _)| *n != a.name);
    st.runs.push((a.name.clone(), run));
    st.cfg.actions.push(a);
    Ok(())
}

/// `leon.on` in a config: there are no events at load time, so the handler would never fire.
fn on_in_config(lua: &Lua, _: MultiValue) -> Result<(), Bad> {
    let at = caller(lua);
    let message = "leon.on: only scripts (`leon do`) get events; this handler never fires".into();
    state(lua).cfg.diagnostics.push(Diagnostic { level: Level::Warning, at, message });
    Ok(())
}

/// `{ name = "type[?]" | schema-table, … }` → one object JSON Schema.
fn args_schema(t: Table) -> Result<serde_json::Value, Bad> {
    let mut props = serde_json::Map::new();
    let mut required = Vec::new();
    for pair in t.pairs::<String, Value>() {
        let (k, v) = pair?;
        let schema = match &v {
            Value::String(s) => {
                let s = s.to_string_lossy();
                let (ty, optional) = s.strip_suffix('?').map_or((s.as_str(), false), |t| (t, true));
                if !optional {
                    required.push(k.clone());
                }
                match ty {
                    "any" => json!({}),
                    "string" | "number" | "integer" | "boolean" | "object" | "array" => json!({ "type": ty }),
                    _ => {
                        return Err(format!(
                            "args.{k}: unknown type {ty:?}; use string, number, integer, boolean, object, array or any (? = optional)"
                        )
                        .into())
                    }
                }
            }
            Value::Table(_) => {
                required.push(k.clone());
                serde_json::to_value(&v).map_err(|e| Bad(format!("args.{k}: {e}")))?
            }
            v => {
                return Err(
                    format!("args.{k}: expected a type name or a JSON Schema table, got {}", v.type_name()).into()
                )
            }
        };
        props.insert(k, schema);
    }
    Ok(json!({ "type": "object", "properties": props, "required": required, "additionalProperties": false }))
}

fn log(lua: &Lua, (a, b): (String, Option<String>)) -> Result<(), Bad> {
    let (level, message) = match b {
        None => (Level::Info, a),
        Some(msg) => {
            let level = match a.as_str() {
                "debug" => Level::Debug,
                "info" => Level::Info,
                "warn" => Level::Warning,
                "error" => Level::Error,
                _ => return Err(format!("unknown level {a:?}; use debug, info, warn or error").into()),
            };
            (level, msg)
        }
    };
    let at = caller(lua);
    if state(lua).live {
        eprintln!("{at}: {}: {message}", level.as_str());
    } else {
        state(lua).cfg.logs.push(Diagnostic { level, at, message });
    }
    Ok(())
}
