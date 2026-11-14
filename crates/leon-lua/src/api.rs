//! The `leon` table for config files, and the sandbox around it.

use std::path::Path;

use mlua::{FromLua, FromLuaMulti, Function, IntoLuaMulti, Lua, LuaSerdeExt, MultiValue, Table, Value};
use serde_json::json;

use crate::keys::{self, KeySpec};
use crate::{Action, Binding, Config, Diagnostic, Level, Mode, Target, Unbind};

/// Load-time state, kept in the Lua state's app data.
#[derive(Default)]
struct State {
    entries: Vec<Entry>,
    next_id: i64,
    /// Everything but `bindings`, which come from `entries` at the end.
    cfg: Config,
}

struct Entry {
    /// Lets `leon.unbind(binding)` find exactly one.
    id: i64,
    b: Binding,
    func: Option<Function>,
}

/// Why a `leon.*` call was skipped.
struct Bad(String);

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

pub(crate) fn run(path: &Path, src: &[u8]) -> mlua::Result<Config> {
    let lua = Lua::new();
    lua.set_app_data(State::default());
    sandbox(&lua, path)?;
    lua.globals().set("leon", leon_table(&lua)?)?;
    let result = lua.load(src).set_name(format!("@{}", path.display())).exec();
    let st = lua.remove_app_data::<State>().expect("leon state");
    result?;
    let mut cfg = st.cfg;
    cfg.bindings = st.entries.into_iter().map(|e| e.b).collect();
    Ok(cfg)
}

fn sandbox(lua: &Lua, path: &Path) -> mlua::Result<()> {
    let dir = path.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let package: Table = lua.globals().get("package")?;
    // shortcut: a config dir whose path holds `?` or `;` breaks these templates; escape them if anyone hits it.
    package.set("path", format!("{0}/?.lua;{0}/?/init.lua", dir.display()))?;
    package.set("cpath", "")?;
    lua.load(
        r#"for _, name in ipairs({ "io.popen", "os.execute" }) do
             local lib, fn = name:match("(%a+)%.(%a+)")
             _G[lib][fn] = function() error(name .. " is not available in leon; use leon.exec", 2) end
           end"#,
    )
    .set_name("=sandbox")
    .exec()
}

fn state(lua: &Lua) -> mlua::AppDataRefMut<'_, State> {
    lua.app_data_mut::<State>().expect("leon state")
}

/// `file:line` of the Lua code calling the running `leon.*` function.
fn caller(lua: &Lua) -> String {
    lua.inspect_stack(1, |d| {
        let src = d.source().source.map(|s| s.trim_start_matches('@').to_string()).unwrap_or_default();
        format!("{src}:{}", d.current_line().unwrap_or(0))
    })
    .unwrap_or_else(|| "?".into())
}

/// A `leon.NAME` function that never raises: a bad call is recorded as an error diagnostic at the
/// caller's line and returns `nil, message`.
fn func<A, R>(lua: &Lua, name: &'static str, f: impl Fn(&Lua, A) -> Result<R, Bad> + 'static) -> mlua::Result<Function>
where
    A: FromLuaMulti,
    R: IntoLuaMulti,
{
    lua.create_function(move |lua, args: MultiValue| {
        match A::from_lua_multi(args, lua).map_err(Bad::from).and_then(|a| f(lua, a)) {
            Ok(r) => r.into_lua_multi(lua),
            Err(Bad(msg)) => {
                let at = caller(lua);
                let message = format!("leon.{name}: {msg}");
                state(lua).cfg.diagnostics.push(Diagnostic { level: Level::Error, at, message });
                (Value::Nil, msg).into_lua_multi(lua)
            }
        }
    })
}

fn field<T: FromLua>(t: &Table, k: &str) -> Result<Option<T>, Bad> {
    t.get::<Option<T>>(k).map_err(|e| Bad(format!("{k}: {e}")))
}

fn required<T: FromLua>(t: &Table, k: &str) -> Result<T, Bad> {
    field(t, k)?.ok_or_else(|| Bad(format!("{k} is required")))
}

fn leon_table(lua: &Lua) -> mlua::Result<Table> {
    let leon = lua.create_table()?;
    leon.set("bind", func(lua, "bind", bind)?)?;
    leon.set("unbind", func(lua, "unbind", unbind)?)?;
    leon.set("mode", func(lua, "mode", mode)?)?;
    leon.set("bindings", func(lua, "bindings", bindings)?)?;
    leon.set("modes", func(lua, "modes", modes)?)?;
    leon.set("action", func(lua, "action", action)?)?;
    leon.set("log", func(lua, "log", log)?)?;
    let json = lua.create_table()?;
    json.set(
        "encode",
        func(lua, "json.encode", |_, (v, opts): (Value, Option<Table>)| {
            let indent = match opts {
                Some(t) => field::<bool>(&t, "indent")?.unwrap_or(false),
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

fn bind(lua: &Lua, (key, target, args): (String, Value, Option<Value>)) -> Result<(), Bad> {
    let key = keys::parse(&key)?;
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
            st.cfg.modes.push(Mode { name: m.clone(), exclusive: false, source: source.clone() });
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
    st.entries.push(Entry { id, b: Binding { key, target, source }, func });
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
    let exclusive = match opts {
        Some(t) => field::<bool>(&t, "exclusive")?.unwrap_or(false),
        None => false,
    };
    let source = caller(lua);
    let mut st = state(lua);
    match st.cfg.modes.iter_mut().find(|m| m.name == name) {
        Some(m) => m.exclusive = exclusive,
        None => st.cfg.modes.push(Mode { name, exclusive, source }),
    }
    Ok(())
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
    let _run: Function = required(&t, "run")?;
    let args = field::<Table>(&t, "args")?.map(args_schema).transpose()?;
    let source = caller(lua);
    let a = Action {
        name,
        title,
        description: field(&t, "description")?,
        category: field(&t, "category")?.unwrap_or_else(|| "Actions".into()),
        keywords: field(&t, "keywords")?.unwrap_or_default(),
        args,
        repeats: field(&t, "repeats")?.unwrap_or(false),
        source,
    };
    let mut st = state(lua);
    if let Some(i) = st.cfg.actions.iter().position(|x| x.name == a.name) {
        let old = st.cfg.actions.remove(i);
        let message = format!("action {:?} redefined (was {}), newest wins", a.name, old.source);
        st.cfg.diagnostics.push(Diagnostic { level: Level::Warning, at: a.source.clone(), message });
    }
    st.cfg.actions.push(a);
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
    state(lua).cfg.logs.push(Diagnostic { level, at, message });
    Ok(())
}
