//! `leon do` scripts: run once top to bottom, then stay alive while event handlers or timers exist.
//! One loop (`pump`) serves `leon.wait`, `leon.sleep` and the keep-alive: it fires due timers and
//! hands events to handlers until its own condition holds.

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use mlua::{Function, IntoLuaMulti, Lua, MultiValue, Table, UserData, UserDataMethods, Value};
use serde_json::json;

use crate::lua_message;
use crate::runtime::{ctx_table, flags, new_state, rt, to_lua, CallCtx, Host};

/// Live timers a script may hold (spec §4).
pub const MAX_TIMERS: usize = 1024;

pub struct ScriptOptions {
    /// The chunk name in error messages: `@path` for a file, `=(command line)` for `-e`.
    pub name: String,
    /// Where `require` looks.
    pub dir: PathBuf,
    /// `leon.args`.
    pub args: serde_json::Value,
    pub session_id: Option<String>,
    pub block_id: Option<String>,
    /// End the script (exit 0) after this long.
    pub run_for: Option<Duration>,
    /// A handler or timer error ends the script with exit 1 (otherwise it is reported and skipped).
    pub fail_fast: bool,
}

struct Timer {
    next: Instant,
    every: Option<Duration>,
    f: Function,
    live: Rc<Cell<bool>>,
}

#[derive(Default)]
struct Script {
    handlers: Vec<(String, Function)>,
    timers: Vec<Timer>,
    fail_fast: bool,
    /// `--for`: when the script ends.
    end: Option<Instant>,
}

fn script(lua: &Lua) -> mlua::AppDataRefMut<'_, Script> {
    lua.app_data_mut::<Script>().expect("script state")
}

struct TimerHandle(Rc<Cell<bool>>);

impl UserData for TimerHandle {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("cancel", |_, t, ()| {
            t.0.set(false);
            Ok(())
        });
        methods.add_method("active", |_, t, ()| Ok(t.0.get()));
    }
}

/// Run a script. Ok(exit code): 0 when it ends, or `leon.stop`'s code. Err: the error that ended it.
pub fn run_script(src: &[u8], host: Box<dyn Host>, opts: ScriptOptions) -> Result<i32, String> {
    let (lua, flags, leon) = new_state(&opts.dir, true, host).map_err(lua_message)?;
    rt(&lua).ctx = CallCtx {
        session_id: opts.session_id.clone(),
        block_id: opts.block_id.clone(),
        client_id: None,
        origin: "script".into(),
    };
    let end = opts.run_for.map(|d| Instant::now() + d);
    lua.set_app_data(Script { fail_fast: opts.fail_fast, end, ..Default::default() });
    let run = || -> mlua::Result<()> {
        functions(&lua, &leon, &opts)?;
        lua.load(src).set_name(&opts.name).exec()?;
        pump(&lua, None, None).map(drop)
    };
    let r = run();
    if let Some(code) = flags.stop.get() {
        return Ok(code);
    }
    r.map(|()| 0).map_err(lua_message)
}

fn functions(lua: &Lua, leon: &Table, opts: &ScriptOptions) -> mlua::Result<()> {
    leon.set("args", to_lua(lua, &opts.args)?)?;
    leon.set("session_id", opts.session_id.clone())?;
    leon.set("block_id", opts.block_id.clone())?;
    leon.set(
        "on",
        lua.create_function(|lua, (name, f): (String, Function)| {
            script(lua).handlers.push((name, f));
            Ok(())
        })?,
    )?;
    leon.set("after", lua.create_function(|lua, (s, f): (f64, Function)| timer(lua, s, f, false))?)?;
    leon.set("every", lua.create_function(|lua, (s, f): (f64, Function)| timer(lua, s, f, true))?)?;
    leon.set("sleep", lua.create_function(|lua, s: f64| pump(lua, Some(Instant::now() + secs(s)), None).map(drop))?)?;
    leon.set(
        "stop",
        lua.create_function(|lua, code: Option<i32>| -> mlua::Result<()> {
            let f = flags(lua);
            f.stop.set(Some(code.unwrap_or(0)));
            Err(f.end("leon.stop"))
        })?,
    )?;
    leon.set("wait", lua.create_function(wait)?)
}

fn secs(s: f64) -> Duration {
    Duration::try_from_secs_f64(s.max(0.0)).unwrap_or(Duration::MAX / 2)
}

fn timer(lua: &Lua, s: f64, f: Function, repeat: bool) -> mlua::Result<TimerHandle> {
    let mut sc = script(lua);
    sc.timers.retain(|t| t.live.get());
    if sc.timers.len() >= MAX_TIMERS {
        return Err(mlua::Error::RuntimeError(format!("a script may hold at most {MAX_TIMERS} live timers")));
    }
    let live = Rc::new(Cell::new(true));
    let every = repeat.then(|| secs(s).max(Duration::from_millis(1)));
    sc.timers.push(Timer { next: Instant::now() + secs(s), every, f, live: live.clone() });
    Ok(TimerHandle(live))
}

/// What `leon.wait` waits for.
struct Want {
    name: String,
    filter: Filter,
}

enum Filter {
    Any,
    Fields(serde_json::Map<String, serde_json::Value>),
    Func(Function),
}

impl Want {
    fn matches(&self, lua: &Lua, ev: &serde_json::Value) -> mlua::Result<bool> {
        if !event_is(ev, &self.name) {
            return Ok(false);
        }
        Ok(match &self.filter {
            Filter::Any => true,
            Filter::Fields(m) => m.iter().all(|(k, v)| ev.get(k) == Some(v) || ev["payload"].get(k) == Some(v)),
            Filter::Func(f) => {
                let v = f.call::<Value>((to_lua(lua, ev)?, ctx_table(lua, None)?))?;
                !matches!(v, Value::Nil | Value::Boolean(false))
            }
        })
    }
}

/// `leon.wait(event[, filter][, timeout])` → `ev, ctx`, or `nil, "timeout"`.
fn wait(lua: &Lua, (name, a, b): (String, Value, Value)) -> mlua::Result<MultiValue> {
    let (filter, timeout) = match a {
        Value::Integer(_) | Value::Number(_) => (Value::Nil, a),
        _ => (a, b),
    };
    let filter = match filter {
        Value::Nil => Filter::Any,
        Value::Function(f) => Filter::Func(f),
        v @ Value::Table(_) => match serde_json::to_value(&v) {
            Ok(serde_json::Value::Object(m)) => Filter::Fields(m),
            _ => Filter::Any, // an empty table serializes as {} or []; either way it filters nothing
        },
        v => return Err(mlua::Error::RuntimeError(format!("leon.wait: bad filter {}", v.type_name()))),
    };
    let until = match timeout {
        Value::Nil => None,
        v => Some(Instant::now() + secs(number(&v)?)),
    };
    match pump(lua, until, Some(&Want { name, filter }))? {
        Some(ev) => (to_lua(lua, &ev)?, ctx_table(lua, None)?).into_lua_multi(lua),
        None => (Value::Nil, "timeout").into_lua_multi(lua),
    }
}

fn number(v: &Value) -> mlua::Result<f64> {
    match v {
        Value::Integer(n) => Ok(*n as f64),
        Value::Number(n) => Ok(*n),
        v => Err(mlua::Error::RuntimeError(format!("leon.wait: timeout must be a number, got {}", v.type_name()))),
    }
}

/// Fire timers and hand events to handlers until `until` passes (None), a `want`ed event arrives
/// (Some), or, with neither, nothing is left to wait for.
fn pump(lua: &Lua, until: Option<Instant>, want: Option<&Want>) -> mlua::Result<Option<serde_json::Value>> {
    let end = script(lua).end;
    loop {
        if let Some(e) = flags(lua).aborted() {
            return Err(e);
        }
        let now = Instant::now();
        if end.is_some_and(|e| now >= e) {
            let f = flags(lua);
            f.stop.set(Some(0));
            return Err(f.end("the script's --for time is up"));
        }
        for (f, live, once) in due(lua, now) {
            if live.get() {
                if once {
                    live.set(false);
                }
                call(lua, &f, "timer", Value::Nil)?;
            }
        }
        let (alive, next) = {
            let s = script(lua);
            (!s.handlers.is_empty() || !s.timers.is_empty(), s.timers.iter().map(|t| t.next).min())
        };
        if want.is_none() && until.is_none() && !alive {
            return Ok(None);
        }
        let now = Instant::now();
        if until.is_some_and(|u| now >= u) {
            return Ok(None);
        }
        let wake = [until, next, end].into_iter().flatten().min();
        let line = rt(lua).host.next_event(wake.map(|w| w.saturating_duration_since(now)));
        let Some(line) = line.map_err(mlua::Error::RuntimeError)? else { continue };
        let ev = unwrap_event(&line);
        if let Some(w) = want {
            if w.matches(lua, &ev)? {
                return Ok(Some(ev));
            }
        }
        let hs: Vec<Function> =
            script(lua).handlers.iter().filter(|(n, _)| event_is(&ev, n)).map(|(_, f)| f.clone()).collect();
        if !hs.is_empty() {
            let v = to_lua(lua, &ev)?;
            for h in hs {
                call(lua, &h, "script", v.clone())?;
            }
        }
    }
}

/// The timers due at `now`: (callback, its live flag, fires once). Repeating ones are rescheduled;
/// missed ticks coalesce into one.
fn due(lua: &Lua, now: Instant) -> Vec<(Function, Rc<Cell<bool>>, bool)> {
    let mut s = script(lua);
    s.timers.retain(|t| t.live.get());
    let mut out = Vec::new();
    s.timers.retain_mut(|t| {
        if t.next > now {
            return true;
        }
        out.push((t.f.clone(), t.live.clone(), t.every.is_none()));
        match t.every {
            Some(i) => {
                t.next = (t.next + i).max(now);
                true
            }
            None => false,
        }
    });
    out
}

/// Run a handler or timer callback. Its error is reported and skipped, unless the run is ending
/// (timeout, `leon.stop`) or `--fail-fast` is on.
fn call(lua: &Lua, f: &Function, origin: &str, ev: Value) -> mlua::Result<()> {
    let r = ctx_table(lua, Some(origin)).and_then(|c| f.call::<()>((c, ev)));
    let Err(e) = r else { return Ok(()) };
    let fl = flags(lua);
    if let Some(e) = fl.aborted() {
        return Err(e);
    }
    let msg = lua_message(e);
    eprintln!("leon: {origin} handler: {msg}");
    if script(lua).fail_fast {
        fl.stop.set(Some(1));
        return Err(fl.end(&format!("--fail-fast: {msg}")));
    }
    Ok(())
}

/// An event line in the spec's CLI JSON shape, unwrapped for Lua: `{type, session_id, payload}`,
/// plus `block_id`, `creator_name` and `name` for block events (whose `type` is their name).
pub(crate) fn unwrap_event(line: &serde_json::Value) -> serde_json::Value {
    let e = &line["event"];
    if e["type"] != "session_event" {
        let sid = e["payload"]["session_id"].as_str().or(e["payload"]["session"]["session_id"].as_str());
        return json!({ "type": e["type"], "session_id": sid, "payload": e["payload"] });
    }
    let (sid, inner) = (&e["payload"]["session_id"], &e["payload"]["event"]);
    if inner["type"] == "block_event" {
        let b = &inner["payload"];
        return json!({ "type": b["name"], "session_id": sid, "block_id": b["block_id"],
                       "creator_name": b["creator_name"], "name": b["name"], "payload": b["payload"] });
    }
    json!({ "type": inner["type"], "session_id": sid, "payload": inner["payload"] })
}

/// Whether `name` (as `leon.on`/`leon.wait` take it) names `ev`: its type, `block_event` for any
/// block event, or a block event's `terminal.<name>` / `<creator>.<name>`.
pub(crate) fn event_is(ev: &serde_json::Value, name: &str) -> bool {
    if ev["type"] == name {
        return true;
    }
    let (Some(n), Some(c)) = (ev["name"].as_str(), ev["creator_name"].as_str()) else { return false };
    name == "block_event"
        || name.strip_prefix("terminal.") == Some(n)
        || name.strip_prefix(c).and_then(|r| r.strip_prefix('.')) == Some(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_events_match_every_spelling() {
        let line = json!({ "seq": 1, "event": { "type": "session_event", "payload": { "session_id": "s",
            "event": { "type": "block_event", "payload": { "block_id": "b", "creator_name": "dev.leon.terminal",
            "name": "child_exited", "payload": { "exit_code": 3 } } } } } });
        let ev = unwrap_event(&line);
        assert_eq!(ev["payload"]["exit_code"], 3);
        for n in ["child_exited", "terminal.child_exited", "dev.leon.terminal.child_exited", "block_event"] {
            assert!(event_is(&ev, n), "{n}");
        }
        assert!(!event_is(&ev, "pwd_changed"));
        let server = unwrap_event(&json!({ "event": { "type": "keymap_changed", "payload": { "generation": 2 } } }));
        assert!(event_is(&server, "keymap_changed") && !event_is(&server, "block_event"));
    }
}
