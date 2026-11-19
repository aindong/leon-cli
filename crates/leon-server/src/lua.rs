//! Lua on the server: the loaded `init.lua` and the methods that use it (`client.action`,
//! `client.key`, `client.keymap`, `server.config_reload`, `server.settings`).
//!
//! Each good load gets its own thread ("leon-lua") owning the Lua state; jobs (an action, a function
//! binding) reach it over a channel and reply on a oneshot. Lua calls server methods in process with
//! `Handle::block_on`. A reload builds a new thread and only a good load replaces the current one, so
//! a failed reload keeps the last good config and a wedged thread is dropped by the next reload.
//! The config sits behind a tokio RwLock held for writing during any load; no handler holds it
//! while it waits on a job, so a reload never queues behind a stuck action.

use std::cell::Cell;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Weak};
use std::time::{Duration, Instant};

use leon_core::ApiError;
use leon_lua::keys;
use leon_lua::{CallCtx, Host, Runtime, Target};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::runtime::Handle;
use tokio::sync::{oneshot, RwLock};

use crate::focus_guard;
use crate::methods::NoParams;
use crate::{Ctx, Later, Registry};

/// An action (or function binding) gets this long from the moment it is asked for
/// (docs/lua/scripts.md, "Limits").
pub const ACTION_LIMIT: Duration = Duration::from_secs(30);

thread_local! {
    static IN_LUA: Cell<bool> = const { Cell::new(false) };
}

/// The current config. `generation` counts good loads.
#[derive(Default)]
pub struct Loaded {
    path: Option<PathBuf>,
    generation: u64,
    /// Counts good loads that changed a `serve_tailscale*` setting: only those re-pin the account
    /// served over Tailscale (tailnet.rs), never an unrelated `leon config set`.
    serve_generation: u64,
    cfg: leon_lua::Config,
    /// None when no config file exists.
    jobs: Option<Jobs>,
}

pub type Shared = Arc<RwLock<Loaded>>;

/// A job, and who it runs for, when a request asked for it (focus_guard.rs).
type Jobs = mpsc::Sender<(Option<focus_guard::Caller>, Job)>;

enum Job {
    Action { name: String, args: Value, ctx: CallCtx, deadline: Instant, reply: oneshot::Sender<Result<Value, String>> },
    Binding { index: usize, ctx: CallCtx, ev: Value, deadline: Instant, reply: oneshot::Sender<Result<bool, String>> },
}

/// Server calls from the Lua thread, in process.
struct InProc {
    ctx: Weak<Ctx>,
    rt: Handle,
}

impl Host for InProc {
    fn call(&mut self, method: &str, params: Value, deadline: Option<Instant>) -> Result<Value, String> {
        let ctx = self.ctx.upgrade().ok_or("the server is stopping")?;
        let call = ctx.call(method, params);
        let r = match deadline {
            None => self.rt.block_on(call),
            Some(d) => self.rt.block_on(async {
                let late = || ApiError::new(504, "timeout", "the action ran out of time during this call");
                tokio::time::timeout_at(d.into(), call).await.unwrap_or_else(|_| Err(late()))
            }),
        };
        r.map_err(|e| e.to_string())
    }
}

fn timed_out(what: &str) -> ApiError {
    ApiError::new(504, "timeout", format!("{what} timed out after {} s", ACTION_LIMIT.as_secs()))
}

fn config_error(msg: impl Into<String>) -> ApiError {
    ApiError::new(400, "config_error", msg)
}

/// Load `path` on a new Lua thread. A missing file is an empty config with no thread.
async fn load(ctx: &Arc<Ctx>, path: PathBuf) -> Result<(leon_lua::Config, Option<Jobs>), ApiError> {
    if !path.exists() {
        return Ok((leon_lua::Config::default(), None));
    }
    let (loaded, result) = oneshot::channel();
    let (jobs, queue) = mpsc::channel();
    let host = InProc { ctx: Arc::downgrade(ctx), rt: Handle::current() };
    let spawned = std::thread::Builder::new().name("leon-lua".into()).spawn(move || {
        IN_LUA.with(|f| f.set(true));
        match Runtime::load(&path, Box::new(host)) {
            Ok((rt, cfg)) => {
                let _ = loaded.send(Ok(cfg));
                for (caller, job) in queue {
                    // A panicking job drops its reply (the caller gets "gone"); the thread lives on.
                    let job = std::panic::AssertUnwindSafe(|| run(&rt, job));
                    let _ = match caller {
                        Some(c) => focus_guard::within(c, || std::panic::catch_unwind(job)),
                        None => std::panic::catch_unwind(job),
                    };
                }
            }
            Err(e) => drop(loaded.send(Err(e.to_string()))),
        }
    });
    spawned.map_err(|e| config_error(format!("can't start the Lua thread: {e}")))?;
    let limit = leon_lua::LOAD_LIMIT + Duration::from_secs(2);
    match tokio::time::timeout(limit, result).await {
        Ok(Ok(Ok(cfg))) => Ok((cfg, Some(jobs))),
        Ok(Ok(Err(e))) => Err(config_error(e)),
        Ok(Err(_)) => Err(config_error("the config load crashed")),
        Err(_) => Err(config_error(format!("the config load timed out after {} s", limit.as_secs()))),
    }
}

fn run(rt: &Runtime, job: Job) {
    match job {
        Job::Action { name, args, ctx, deadline, reply } => {
            drop(reply.send(rt.run_action(&name, args, &ctx, deadline)))
        }
        Job::Binding { index, ctx, ev, deadline, reply } => drop(reply.send(rt.run_binding(index, &ctx, ev, deadline))),
    }
}

/// Reload into `cur` (held for writing). On success bump the generation and tell `--all` subscribers.
async fn reload(ctx: &Arc<Ctx>, cur: &mut Loaded) -> Result<Reloaded, ApiError> {
    let path = cur.path.clone().or_else(leon_lua::config_path).ok_or_else(|| config_error("no config path"))?;
    let (cfg, jobs) = load(ctx, path.clone()).await?;
    let changed = cfg.settings != cur.cfg.settings;
    let serve = |s: &leon_lua::Settings| (s.serve_tailscale, s.serve_tailscale_port, s.serve_tailscale_tags.clone());
    let serve_generation = cur.serve_generation + u64::from(serve(&cfg.settings) != serve(&cur.cfg.settings));
    *cur = Loaded { path: Some(path.clone()), generation: cur.generation + 1, serve_generation, cfg, jobs };
    let lines = cur.cfg.settings.scrollback;
    if ctx.scrollback.swap(lines, std::sync::atomic::Ordering::SeqCst) != lines {
        // under the state lock, as a new block is started: none misses the new cap
        let st = ctx.state();
        st.sessions.iter().flat_map(|s| &s.blocks).for_each(|b| b.terminal.set_scrollback(lines));
    }
    if changed {
        let view = serde_json::to_value(crate::settings::view(&cur.cfg)).expect("json");
        ctx.state().emit_global("settings_changed", view);
    }
    let km = keymap(cur);
    let mut h = std::collections::hash_map::DefaultHasher::new();
    serde_json::to_string(&(&km.bindings, &km.modes)).expect("json").hash(&mut h);
    ctx.state()
        .emit_global("keymap_changed", json!({ "generation": km.generation, "hash": format!("{:016x}", h.finish()) }));
    let c = &cur.cfg;
    Ok(Reloaded {
        generation: cur.generation,
        path: path.display().to_string(),
        bindings: c.bindings.len(),
        modes: c.modes.len(),
        actions: c.actions.len(),
        diagnostics: c.diagnostics.iter().map(|d| d.to_string()).collect(),
        logs: c.logs.iter().map(|l| format!("{}: log.{}: {}", l.at, l.level.as_str(), l.message)).collect(),
    })
}

impl Ctx {
    /// Load the config at server start, from `path` (else `$LEON_CONFIG`, XDG, `~/.config`). Call
    /// inside the runtime; the server serves meanwhile, and config methods wait for the load.
    pub fn start_config(self: &Arc<Self>, path: Option<PathBuf>) {
        let mut guard = self.lua.clone().try_write_owned().expect("nothing holds the config at start");
        guard.path = path.or_else(leon_lua::config_path);
        let ctx = self.clone();
        tokio::spawn(async move {
            if let Err(e) = reload(&ctx, &mut guard).await {
                eprintln!("leon: config not loaded: {}", e.message);
            }
        });
    }
}

impl Ctx {
    /// The effective settings, and how many loads changed a `serve_tailscale*` setting.
    pub(crate) async fn settings(&self) -> (leon_lua::Settings, u64) {
        let l = self.lua.read().await;
        (l.cfg.settings.clone(), l.serve_generation)
    }

    /// The config's resume setting, once the start-up load (`start_config`) has finished. Waits at
    /// most a second: nothing is served until the restore that asks is done. Past it, "never": the
    /// setting is unknown, so nothing is typed.
    pub async fn resume_policy(&self) -> leon_lua::Resume {
        match tokio::time::timeout(Duration::from_secs(1), self.lua.read()).await {
            Ok(loaded) => loaded.cfg.settings.resume,
            Err(_) => {
                eprintln!("leon: warning: the config is slow to load; restored blocks don't offer to resume");
                leon_lua::Resume::Never
            }
        }
    }
}

/// Ask the config's Lua thread to run a job; reply by `deadline` (plus a second for the reply).
async fn ask<T: Send + 'static>(
    jobs: Option<Jobs>,
    what: &str,
    deadline: Instant,
    job: impl FnOnce(oneshot::Sender<Result<T, String>>) -> Job,
) -> Result<T, ApiError> {
    let (tx, rx) = oneshot::channel();
    let gone = || ApiError::new(503, "unavailable", "the config's Lua state is gone; run `leon config reload`");
    jobs.ok_or_else(gone)?.send((focus_guard::caller(), job(tx))).map_err(|_| gone())?;
    match tokio::time::timeout_at((deadline + Duration::from_secs(1)).into(), rx).await {
        Ok(Ok(Ok(v))) => Ok(v),
        Ok(Ok(Err(msg))) if msg == leon_lua::TIMED_OUT => Err(timed_out(what)),
        Ok(Ok(Err(msg))) => Err(ApiError::new(500, "action_failed", msg)),
        Ok(Err(_)) => Err(gone()),
        Err(_) => Err(timed_out(what)),
    }
}

fn refuse_in_lua(method: &str) -> Result<(), ApiError> {
    match IN_LUA.with(Cell::get) {
        true => {
            Err(ApiError::bad_request(format!("{method} from inside an action would wait on itself; use leon.invoke")))
        }
        false => Ok(()),
    }
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ActionCall {
    /// A config action's name.
    pub action: String,
    pub args: Option<Value>,
    /// `ctx.origin` for the action (default `api`).
    pub origin: Option<String>,
    pub session_id: Option<String>,
    pub block_id: Option<String>,
}

#[derive(Serialize, JsonSchema)]
pub struct ActionResult {
    /// What the action's `run` returned.
    pub result: Value,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct KeyPress {
    pub session_id: String,
    /// One key in binding syntax (`ctrl+x`).
    pub key: String,
    /// The block that has the key; default the active window's focused block.
    pub block_id: Option<String>,
    /// The keys of a sequence pressed so far (`ctrl+b`), from a `pending` reply.
    pub prefix: Option<String>,
}

#[derive(Serialize, JsonSchema)]
pub struct KeyResult {
    /// A binding took the key.
    pub handled: bool,
    /// The keys so far start a longer sequence: send the next with `prefix`.
    pub pending: bool,
    pub prefix: Option<String>,
    /// The action that ran, for an action binding.
    pub action: Option<String>,
}

#[path = "lua_keymap.rs"]
mod keymap_view;
pub use keymap_view::*;

async fn action(ctx: Arc<Ctx>, p: ActionCall) -> Result<ActionResult, ApiError> {
    let deadline = Instant::now() + ACTION_LIMIT;
    let jobs = {
        let cur = ctx.lua.read().await;
        if !cur.cfg.actions.iter().any(|a| a.name == p.action) {
            return Err(ApiError::not_found(format!("no action {} in the config", p.action)));
        }
        cur.jobs.clone()
    };
    let c = CallCtx {
        session_id: p.session_id,
        block_id: p.block_id,
        client_id: None,
        origin: p.origin.unwrap_or_else(|| "api".into()),
    };
    let (name, args) = (p.action, p.args.unwrap_or_else(|| json!({})));
    let what = format!("action {name}");
    let result = ask(jobs, &what, deadline, |reply| Job::Action { name, args, ctx: c, deadline, reply }).await?;
    Ok(ActionResult { result })
}

async fn key(ctx: Arc<Ctx>, p: KeyPress) -> Result<KeyResult, ApiError> {
    let deadline = Instant::now() + ACTION_LIMIT;
    let full = match &p.prefix {
        Some(pre) => format!("{pre}>{}", p.key),
        None => p.key.clone(),
    };
    let spec = keys::parse(&full).map_err(ApiError::bad_request)?;
    if spec.mode.is_some() {
        return Err(ApiError::bad_request("client.key doesn't do modes yet; send the key alone"));
    }
    let (bindings, jobs) = {
        let cur = ctx.lua.read().await;
        (cur.cfg.bindings.clone(), cur.jobs.clone())
    };
    let free = || bindings.iter().enumerate().filter(|(_, b)| b.key.mode.is_none());
    if free().any(|(_, b)| b.key.seq.len() > spec.seq.len() && b.key.seq.starts_with(&spec.seq)) {
        return Ok(KeyResult { handled: false, pending: true, prefix: Some(spec.sequence()), action: None });
    }
    let c = CallCtx {
        session_id: Some(p.session_id.clone()),
        block_id: p.block_id.clone(),
        client_id: None,
        origin: "key".into(),
    };
    let exact: Vec<_> = free().filter(|(_, b)| b.key.seq == spec.seq).collect();
    for (index, b) in exact.into_iter().rev() {
        match &b.target {
            Target::Action { name, args } => {
                let (name, args, c) = (name.clone(), args.clone().unwrap_or_else(|| json!({})), c.clone());
                let what = format!("action {name}");
                let n = name.clone();
                ask(jobs.clone(), &what, deadline, |reply| Job::Action { name: n, args, ctx: c, deadline, reply })
                    .await?;
                return Ok(KeyResult { handled: true, pending: false, prefix: None, action: Some(name) });
            }
            Target::Function => {
                let ev = json!({ "keybinding": b.key.sequence(), "prefix": p.prefix });
                let c = c.clone();
                let what = format!("the binding at {}", b.source);
                if ask(jobs.clone(), &what, deadline, |reply| Job::Binding { index, ctx: c, ev, deadline, reply })
                    .await?
                {
                    return Ok(KeyResult { handled: true, pending: false, prefix: None, action: None });
                }
            }
        }
    }
    if p.prefix.is_none() {
        let block_id = match p.block_id {
            Some(b) => b,
            None => {
                let st = ctx.state();
                let s = st.session(&p.session_id)?;
                let focus = s.windows.get(s.active).and_then(|w| w.focus.clone());
                focus.ok_or_else(|| ApiError::not_found("no focused block to type into"))?
            }
        };
        let event = json!({ "type": "key", "key": p.key });
        ctx.call("block.event", json!({ "session_id": p.session_id, "block_id": block_id, "event": event })).await?;
    }
    Ok(KeyResult { handled: false, pending: false, prefix: None, action: None })
}

pub(crate) fn add(r: &mut Registry) {
    r.add_deferred(
        "client.action",
        "Run a config action (`leon.action` in init.lua) on the server and return what it returned. \
         It gets 30 s; `error(msg, 0)` fails it with msg.",
        |ctx, p: ActionCall| {
            refuse_in_lua("client.action")?;
            Ok(Box::pin(action(ctx.clone(), p)) as Later<ActionResult>)
        },
    );
    r.add_deferred(
        "client.key",
        "Press a key through the config's bindings: an action binding runs, a function binding runs \
         and handles it if it returns true, a sequence's prefix replies `pending`; an unhandled key \
         is typed into the block. Modes are not supported yet.",
        |ctx, p: KeyPress| {
            refuse_in_lua("client.key")?;
            Ok(Box::pin(key(ctx.clone(), p)) as Later<KeyResult>)
        },
    );
    r.add_deferred(
        "client.keymap",
        "The server's resolved key bindings, modes and config actions, with the keymap generation.",
        |ctx, _: NoParams| {
            let ctx = ctx.clone();
            Ok(Box::pin(async move { Ok(keymap(&*ctx.lua.read().await)) }) as Later<Keymap>)
        },
    );
    r.add_deferred(
        "server.settings",
        "The effective settings from init.lua (theme, font, notifications, copy_on_select, resume), \
         defaults filled in, and where each was set. A reload that changes them emits \
         `settings_changed` with the same object to `--all` subscribers.",
        |ctx, _: NoParams| {
            let ctx = ctx.clone();
            Ok(Box::pin(async move { Ok(crate::settings::view(&ctx.lua.read().await.cfg)) })
                as Later<crate::settings::SettingsView>)
        },
    );
    r.add_deferred(
        "server.config_reload",
        "Re-read init.lua. A failed load keeps the last good config; a good one bumps the keymap \
         generation and emits `keymap_changed`, and `settings_changed` if a setting changed. \
         Local socket only.",
        |ctx, _: NoParams| {
            let ctx = ctx.clone();
            Ok(Box::pin(async move { reload(&ctx, &mut *ctx.lua.write().await).await }) as Later<Reloaded>)
        },
    );
}
