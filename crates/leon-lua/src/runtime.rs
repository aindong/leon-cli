//! Lua at runtime: the [`Host`] a Lua state reaches the server through, the config [`Runtime`] the
//! server keeps for actions and function bindings, and the `leon.*` calls configs and scripts share
//! (`call`, `session.*`, `block.call`, `invoke`, `layout.*`).
//!
//! Time limits: an instruction hook raises once the deadline in [`Flags`] passes, and keeps raising
//! while `abort` is set; the sandbox's `pcall` family re-raises it (sandbox.rs), so Lua can't swallow it.

use std::cell::{Cell, RefCell};
use std::path::Path;
use std::rc::Rc;
use std::time::{Duration, Instant};

use mlua::serde::SerializeOptions;
use mlua::{Function, HookTriggers, IntoLuaMulti, Lua, LuaSerdeExt, MultiValue, Table, Value, VmState};
use serde_json::json;

use crate::api::{self, Bad};
use crate::{lua_message, Config, Diagnostic, Level, LoadError};

/// How deep `leon.invoke` may nest (spec §4).
pub const MAX_DEPTH: usize = 16;
/// A config load gets this long.
pub const LOAD_LIMIT: Duration = Duration::from_secs(5);
/// The server's config state may use this much memory.
const MEMORY_LIMIT: usize = 256 << 20;
/// The error a run past its deadline fails with.
pub const TIMED_OUT: &str = "timed out";

/// Seconds from Lua as a `Duration`: negative or NaN is 0.
/// shortcut: capped at ~9.5 years so `Instant + d` can't overflow; nobody waits longer.
pub(crate) fn secs(s: f64) -> Duration {
    Duration::from_secs_f64(if s > 0.0 { s.min(3e8) } else { 0.0 })
}

/// What a Lua state calls the server through: in process on the server, over the socket in `leon do`.
pub trait Host {
    /// Call a server method by full name. The running action must be done by `deadline`.
    fn call(
        &mut self,
        method: &str,
        params: serde_json::Value,
        deadline: Option<Instant>,
    ) -> Result<serde_json::Value, String>;

    /// The next pushed event line (`{"event":…,"seq":N}`); `Ok(None)` once `timeout` passes.
    fn next_event(&mut self, timeout: Option<Duration>) -> Result<Option<serde_json::Value>, String> {
        let _ = timeout;
        Err("no event stream here".into())
    }
}

/// The `ctx` an action, binding or handler gets (spec §4).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CallCtx {
    pub session_id: Option<String>,
    pub block_id: Option<String>,
    pub client_id: Option<String>,
    /// `key|palette|cli|api|script|timer`.
    pub origin: String,
}

/// Shared with the hook and the sandbox's `pcall` wrappers.
#[derive(Default)]
pub(crate) struct Flags {
    pub deadline: Cell<Option<Instant>>,
    /// Set once the run must end (timed out, `leon.stop`); the hook and `pcall` re-raise it.
    pub abort: RefCell<Option<String>>,
    /// `leon.stop(code)`.
    pub stop: Cell<Option<i32>>,
}

impl Flags {
    pub fn aborted(&self) -> Option<mlua::Error> {
        self.abort.borrow().clone().map(mlua::Error::RuntimeError)
    }

    pub fn end(&self, why: &str) -> mlua::Error {
        *self.abort.borrow_mut() = Some(why.into());
        mlua::Error::RuntimeError(why.into())
    }
}

pub(crate) struct Rt {
    pub host: Box<dyn Host>,
    pub ctx: CallCtx,
    pub depth: usize,
    pub flags: Rc<Flags>,
    pub script: bool,
    /// `leon.exec.start` children, killed when the action ends.
    pub procs: Vec<std::sync::Weak<crate::exec::Proc>>,
}

pub(crate) fn rt(lua: &Lua) -> mlua::AppDataRefMut<'_, Rt> {
    lua.app_data_mut::<Rt>().expect("leon runtime")
}

pub(crate) fn flags(lua: &Lua) -> Rc<Flags> {
    rt(lua).flags.clone()
}

/// A sandboxed state with the `leon` table, for a config (`script` false) or a script.
pub(crate) fn new_state(dir: &Path, script: bool, host: Box<dyn Host>) -> mlua::Result<(Lua, Rc<Flags>, Table)> {
    let lua = Lua::new();
    let flags = Rc::new(Flags::default());
    lua.set_app_data(api::State { live: script, ..Default::default() });
    lua.set_app_data(Rt { host, ctx: CallCtx::default(), depth: 0, flags: flags.clone(), script, procs: vec![] });
    crate::sandbox::sandbox(&lua, dir, &flags)?;
    let leon = api::leon_table(&lua, script)?;
    shared(&lua, &leon)?;
    crate::exec::add(&lua, &leon)?;
    lua.globals().set("leon", leon.clone())?;
    lua.set_hook(HOOK, hook(flags.clone()))?;
    // Threads don't inherit the hook, so every coroutine is made here and hooked.
    let f = flags.clone();
    let create = lua.create_function(move |lua, func: Function| {
        let th = lua.create_thread(func)?;
        th.set_hook(HOOK, hook(f.clone()))?;
        Ok(th)
    })?;
    lua.load(
        r#"local create, resume = ...
           local function ret(ok, ...)
             if not ok then error((...), 0) end
             return ...
           end
           coroutine.create = create
           coroutine.wrap = function(f)
             local co = create(f)
             return function(...) return ret(resume(co, ...)) end
           end"#,
    )
    .set_name("=sandbox")
    .call::<()>((create, lua.globals().get::<Table>("coroutine")?.get::<Function>("resume")?))?;
    Ok((lua, flags, leon))
}

const HOOK: HookTriggers = HookTriggers::new().every_nth_instruction(1000);

/// Raise once the deadline passes, and keep raising while the run is ending.
fn hook(f: Rc<Flags>) -> impl Fn(&Lua, &mlua::debug::Debug) -> mlua::Result<VmState> + 'static {
    move |_, _| {
        if f.abort.borrow().is_none() && f.deadline.get().is_none_or(|d| Instant::now() < d) {
            return Ok(VmState::Continue);
        }
        let why = f.abort.borrow().clone().unwrap_or_else(|| TIMED_OUT.into());
        Err(f.end(&why))
    }
}

/// JSON into Lua, with `null` as `nil`.
pub(crate) fn to_lua(lua: &Lua, v: &serde_json::Value) -> mlua::Result<Value> {
    lua.to_value_with(v, SerializeOptions::new().serialize_none_to_null(false).serialize_unit_to_null(false))
}

/// Lua into JSON; `nil` is `{}` (no arguments).
pub(crate) fn to_json(v: &Value) -> Result<serde_json::Value, Bad> {
    match v {
        Value::Nil => Ok(json!({})),
        v => serde_json::to_value(v).map_err(|e| Bad(format!("not JSON: {e}"))),
    }
}

pub(crate) fn ctx_table(lua: &Lua, origin: Option<&str>) -> mlua::Result<Table> {
    let c = rt(lua).ctx.clone();
    let t = lua.create_table()?;
    t.set("session_id", c.session_id)?;
    t.set("block_id", c.block_id)?;
    t.set("client_id", c.client_id)?;
    t.set("origin", origin.unwrap_or(&c.origin))?;
    Ok(t)
}

/// A server call from Lua: `result` or `nil, err`, never raising. At config load time there is no
/// client yet, so it fails with a warning.
pub(crate) fn api_call(lua: &Lua, method: &str, params: serde_json::Value) -> mlua::Result<MultiValue> {
    if !api::state(lua).live {
        let at = api::caller(lua);
        let message = format!("leon.call({method}): no client at load time; call it from an action");
        api::state(lua).cfg.diagnostics.push(Diagnostic { level: Level::Warning, at, message });
        return (Value::Nil, "no client at load time").into_lua_multi(lua);
    }
    let deadline = flags(lua).deadline.get();
    let r = rt(lua).host.call(method, params, deadline);
    match r {
        Ok(v) => to_lua(lua, &v)?.into_lua_multi(lua),
        Err(e) => (Value::Nil, e).into_lua_multi(lua),
    }
}

/// `params` as an object with `key` filled from the current ctx when absent.
fn fill(mut params: serde_json::Value, key: &str, value: Option<String>) -> Result<serde_json::Value, Bad> {
    let obj = params.as_object_mut().ok_or_else(|| Bad("arguments must be a table".into()))?;
    if !obj.contains_key(key) {
        if let Some(v) = value {
            obj.insert(key.into(), v.into());
        }
    }
    Ok(params)
}

fn shared(lua: &Lua, leon: &Table) -> mlua::Result<()> {
    leon.set(
        "call",
        api::func(lua, "call", |lua, (method, args): (String, Value)| Ok(api_call(lua, &method, to_json(&args)?)?))?,
    )?;
    for (name, method) in
        [("float", "session.float_block"), ("toggle", "session.toggle"), ("dock", "session.dock_block")]
    {
        leon.set(
            name,
            api::func(lua, name, move |lua, args: Value| {
                let c = rt(lua).ctx.clone();
                let mut params = fill(to_json(&args)?, "session_id", c.session_id)?;
                let mut method = method;
                if name == "float" {
                    if params.as_object_mut().unwrap().remove("new").is_some_and(|v| v == true) {
                        method = "session.new_floating";
                    } else {
                        params = fill(params, "block_id", c.block_id)?;
                    }
                } else if name == "dock" {
                    params = fill(params, "block_id", c.block_id)?;
                }
                Ok(api_call(lua, method, params)?)
            })?,
        )?;
    }
    let session = lua.create_table()?;
    let meta = lua.create_table()?;
    meta.set(
        "__index",
        lua.create_function(|lua, (_, name): (Value, String)| {
            let method = format!("session.{name}");
            api::func(lua, "session", move |lua, args: Value| {
                let sid = rt(lua).ctx.session_id.clone();
                Ok(api_call(lua, &method, fill(to_json(&args)?, "session_id", sid)?)?)
            })
        })?,
    )?;
    session.set_metatable(Some(meta))?;
    leon.set("session", session)?;
    let block = lua.create_table()?;
    block.set(
        "call",
        api::func(lua, "block.call", |lua, (creator, method, args): (String, String, Value)| {
            let c = rt(lua).ctx.clone();
            let params = json!({ "session_id": c.session_id, "block_id": c.block_id,
                                 "method": format!("{creator}.{method}"), "args": to_json(&args)? });
            Ok(api_call(lua, "block.method", params)?)
        })?,
    )?;
    leon.set("block", block)?;
    leon.set("invoke", lua.create_function(invoke)?)?;
    let layout: Table = lua
        .load(
            r#"return {
                 block = function(t) return { block = t } end,
                 horizontal = function(r, a, b) return { split = { direction = "horizontal", ratio = r, before = a, after = b } } end,
                 vertical = function(r, a, b) return { split = { direction = "vertical", ratio = r, before = a, after = b } } end,
               }"#,
        )
        .set_name("=leon.layout")
        .eval()?;
    leon.set("layout", layout)
}

/// `leon.invoke(name[, args])`: a config action (or a script's own) in this state, else, from a
/// script, the server's through `client.action`. `result` or `nil, err`; a timeout still ends the run.
fn invoke(lua: &Lua, (name, args): (String, Value)) -> mlua::Result<MultiValue> {
    let found = api::state(lua).runs.iter().find(|(n, _)| *n == name).map(|(_, f)| f.clone());
    let Some(f) = found else {
        if !rt(lua).script {
            return (Value::Nil, format!("leon.invoke: no action {name}")).into_lua_multi(lua);
        }
        let args = match to_json(&args) {
            Ok(a) => a,
            Err(Bad(e)) => return (Value::Nil, e).into_lua_multi(lua),
        };
        let c = rt(lua).ctx.clone();
        let params = json!({ "action": name, "args": args, "session_id": c.session_id,
                             "block_id": c.block_id, "origin": "script" });
        let r = api_call(lua, "client.action", params)?;
        return match r.front() {
            Some(Value::Table(t)) => t.get::<Value>("result")?.into_lua_multi(lua),
            _ => Ok(r),
        };
    };
    if rt(lua).depth >= MAX_DEPTH {
        let e = format!("leon.invoke({name}): actions nest at most {MAX_DEPTH} deep");
        return (Value::Nil, e).into_lua_multi(lua);
    }
    rt(lua).depth += 1;
    let args = if args.is_nil() { Value::Table(lua.create_table()?) } else { args };
    let r = ctx_table(lua, None).and_then(|c| f.call::<Value>((c, args)));
    rt(lua).depth -= 1;
    if let Some(e) = flags(lua).aborted() {
        return Err(e);
    }
    match r {
        Ok(v) => v.into_lua_multi(lua),
        Err(e) => (Value::Nil, lua_message(e)).into_lua_multi(lua),
    }
}

/// SIGKILL what `leon.exec.start` left running in the action (or load) that just ended.
fn kill_procs(lua: &Lua) {
    let procs = std::mem::take(&mut rt(lua).procs);
    procs.iter().filter_map(std::sync::Weak::upgrade).for_each(|p| p.kill(libc::SIGKILL));
}

/// A loaded config whose Lua state stays alive to run its actions and function bindings.
/// Not `Send`: keep it on one thread.
pub struct Runtime {
    lua: Lua,
    flags: Rc<Flags>,
}

impl Runtime {
    /// Run `path` top to bottom (at most [`LOAD_LIMIT`]) like [`crate::load`], keeping the state.
    pub fn load(path: &Path, host: Box<dyn Host>) -> Result<(Runtime, Config), LoadError> {
        let src = std::fs::read(path).map_err(|e| LoadError(format!("cannot read {}: {e}", path.display())))?;
        let dir = path.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
        let (lua, flags, _) = new_state(dir, false, host).map_err(|e| LoadError(lua_message(e)))?;
        lua.set_memory_limit(MEMORY_LIMIT).map_err(|e| LoadError(lua_message(e)))?;
        flags.deadline.set(Some(Instant::now() + LOAD_LIMIT));
        let r = api::exec_chunk(&lua, path, &src);
        flags.deadline.set(None);
        kill_procs(&lua);
        if flags.abort.take().is_some() {
            let secs = LOAD_LIMIT.as_secs();
            return Err(LoadError(format!("{}: the config load timed out after {secs} s", path.display())));
        }
        r.map_err(|e| LoadError(lua_message(e)))?;
        api::state(&lua).live = true;
        let cfg = api::config(&lua);
        Ok((Runtime { lua, flags }, cfg))
    }

    /// Run action `name` with `args`; its return value is the result. Fails with the action's
    /// error message, or "timed out" once `deadline` passes.
    pub fn run_action(
        &self,
        name: &str,
        args: serde_json::Value,
        ctx: &CallCtx,
        deadline: Instant,
    ) -> Result<serde_json::Value, String> {
        let f = api::state(&self.lua).runs.iter().find(|(n, _)| n == name).map(|(_, f)| f.clone());
        let f = f.ok_or_else(|| format!("no action {name}"))?;
        let args = if args.is_null() { json!({}) } else { args };
        let r = self.run(ctx, deadline, |lua| f.call::<Value>((ctx_table(lua, None)?, to_lua(lua, &args)?)));
        r.and_then(|v| serde_json::to_value(&v).map_err(|e| format!("action {name} returned no JSON: {e}")))
    }

    /// Run the function binding at `index` in the `Config`'s bindings with `ev`. True if it
    /// returned `true` (handled).
    pub fn run_binding(
        &self,
        index: usize,
        ctx: &CallCtx,
        ev: serde_json::Value,
        deadline: Instant,
    ) -> Result<bool, String> {
        let f = api::state(&self.lua).entries.get(index).and_then(|e| e.func.clone());
        let f = f.ok_or_else(|| format!("binding {index} is not a function"))?;
        let r = self.run(ctx, deadline, |lua| f.call::<Value>((ctx_table(lua, None)?, to_lua(lua, &ev)?)));
        r.map(|v| matches!(v, Value::Boolean(true)))
    }

    fn run(
        &self,
        ctx: &CallCtx,
        deadline: Instant,
        f: impl FnOnce(&Lua) -> mlua::Result<Value>,
    ) -> Result<Value, String> {
        {
            let mut rt = rt(&self.lua);
            rt.ctx = ctx.clone();
            rt.depth = 1;
        }
        if Instant::now() >= deadline {
            return Err(TIMED_OUT.into()); // it waited in the queue too long: don't start it
        }
        self.flags.deadline.set(Some(deadline));
        let r = f(&self.lua);
        self.flags.deadline.set(None);
        kill_procs(&self.lua);
        match (self.flags.abort.take(), r) {
            (Some(why), _) => Err(why),
            (None, r) => r.map_err(lua_message),
        }
    }
}
