//! The leon server: a self-describing method registry over the model, served on a Unix socket.
//! The socket loop (`serve`) takes a `Ctx` the caller built, so the app can embed the same state.

mod agents;
mod blocks;
pub mod contract;
pub mod events;
mod focus_guard;
mod layout;
pub mod lua;
pub mod methods;
mod overlays;
pub mod persist;
pub mod registry;
mod restore;
pub mod resume;
pub mod screens;
pub mod serve;
pub mod settings;
pub mod tailnet;
mod web;
mod web_tls;
mod websocket;

use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, Weak};

use leon_core::State;

use events::Subscriber;

pub use contract::API_VERSION;
pub use registry::{Later, Registry};

/// Facts about this server process, reported by `server.status`.
#[derive(Debug, Clone)]
pub struct Info {
    pub pid: u32,
    pub version: &'static str,
    pub socket: String,
}

/// Everything a method handler can reach.
pub struct Ctx {
    state: Mutex<State>,
    pub info: Info,
    pub registry: Registry,
    /// Started on demand: exits once idle. `server.persist` clears it.
    pub autostarted: AtomicBool,
    /// Bumped whenever a block's program exits; `block.wait` watches it.
    exited: tokio::sync::watch::Sender<u64>,
    /// Set (under the state lock) once `server.stop` accepted; no session is created after it.
    pub stopping: AtomicBool,
    /// Ends the socket loop (`serve_on`): `server.stop`, or an embedding app quitting.
    pub shutdown: tokio::sync::Notify,
    /// Attached connections. Only touched with the state lock held, so events and snapshots line up.
    subscribers: Mutex<Vec<Weak<Subscriber>>>,
    /// Resumable event streams by token (resume.rs).
    resumable: Mutex<std::collections::HashMap<String, resume::Slot>>,
    /// Where sessions are saved when the server stops; None when they aren't (`server.status`).
    pub state_file: Mutex<Option<std::path::PathBuf>>,
    /// Set by an accepted `server.stop {discard: true}`: the stop saves nothing and forgets the save.
    pub discard: AtomicBool,
    /// The loaded `init.lua` (lua.rs).
    lua: lua::Shared,
    /// When this server started, in seconds since the Unix epoch (`server.status`).
    pub started_at: u64,
    /// The file its output goes to, when it was started with one (`server.status`).
    pub log: Option<String>,
    /// Serving over Tailscale (tailnet.rs).
    pub tailnet: tailnet::Tailnet,
    /// Set on top of this process's environment for every pane, under leon's own variables: what
    /// an embedding test gives its shells (a plain `sh`, a private home). Empty in leon's server.
    pub pane_env: Vec<(String, String)>,
    /// Each window's geometry as a client last drew it (`client.geometry`): the most recent report
    /// wins when several apps are attached. Runtime only, never saved.
    geometry: Mutex<std::collections::HashMap<String, leon_core::overlays::Geometry>>,
    /// Closed terminals' sessions, for the permission guard (agents.rs).
    pub(crate) gone: agents::Gone,
    /// The `scrollback` setting: lines of history a terminal keeps (lua.rs sets it on a reload).
    scrollback: std::sync::atomic::AtomicUsize,
}

/// The locked state. Dropping it hands the events the holder caused to every subscriber while the
/// lock is still held, so each subscriber gets them in the order of the changes.
pub struct StateGuard<'a> {
    state: MutexGuard<'a, State>,
    subscribers: &'a Mutex<Vec<Weak<Subscriber>>>,
}

impl Deref for StateGuard<'_> {
    type Target = State;
    fn deref(&self) -> &State {
        &self.state
    }
}

impl DerefMut for StateGuard<'_> {
    fn deref_mut(&mut self) -> &mut State {
        &mut self.state
    }
}

impl Drop for StateGuard<'_> {
    fn drop(&mut self) {
        self.state.keep_keys_off_held(focus_guard::person_moves());
        let events = self.state.take_events();
        if events.is_empty() {
            return;
        }
        let mut subs = events::lock(self.subscribers);
        subs.retain(|s| s.strong_count() > 0);
        for sub in subs.iter().filter_map(Weak::upgrade) {
            events.iter().for_each(|e| sub.offer(e));
        }
    }
}

impl Ctx {
    pub fn new(info: Info, autostarted: bool) -> Self {
        Ctx {
            state: Mutex::default(),
            info,
            registry: methods::registry(),
            autostarted: AtomicBool::new(autostarted),
            exited: tokio::sync::watch::channel(0).0,
            stopping: AtomicBool::new(false),
            shutdown: tokio::sync::Notify::new(),
            subscribers: Mutex::default(),
            resumable: Mutex::default(),
            state_file: Mutex::default(),
            discard: AtomicBool::new(false),
            lua: lua::Shared::default(),
            started_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()),
            log: None,
            tailnet: tailnet::Tailnet::default(),
            pane_env: Vec::new(),
            geometry: Mutex::default(),
            gone: agents::Gone::default(),
            scrollback: leon_core::terminal::SCROLLBACK.into(),
        }
    }

    /// What a terminal of block `block_id` runs with on top of this process's environment: the
    /// embedder's `pane_env`, then leon's own variables, and a PATH that finds `leon`.
    pub(crate) fn env_for(&self, session_id: &str, block_id: &str) -> Vec<(String, String)> {
        let mut env = self.pane_env.clone(); // leon's variables after it, so they win
        env.extend(
            [
                ("LEON_SESSION", session_id),
                ("LEON_BLOCK", block_id),
                ("LEON_SERVER", self.info.socket.as_str()),
                ("TERM", "xterm-256color"),
            ]
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string())),
        );
        // `leon` resolves in every terminal, even when the user's PATH lacks where it was installed
        if let Some(dir) = std::env::current_exe().ok().and_then(|p| p.parent().map(std::path::PathBuf::from)) {
            let path = std::env::var_os("PATH")
                .filter(|p| !p.is_empty())
                .unwrap_or_else(|| "/usr/bin:/bin:/usr/sbin:/sbin".into());
            // no empty entries: one would mean the current directory
            let entries =
                std::iter::once(dir.clone()).chain(std::env::split_paths(&path).filter(|p| !p.as_os_str().is_empty()));
            // join_paths refuses a dir with ':' in it; then the terminal keeps the server's PATH
            let joined = std::env::join_paths(entries).ok().and_then(|p| p.into_string().ok());
            if let (Some(dir), Some(path)) = (dir.to_str(), joined) {
                env.extend([("LEON_BIN_DIR".to_string(), dir.to_string()), ("PATH".to_string(), path)]);
            }
        }
        env
    }

    /// Window `window_id`'s geometry as a client last reported it, if one has.
    pub fn geometry(&self, window_id: &str) -> Option<leon_core::overlays::Geometry> {
        events::lock(&self.geometry).get(window_id).copied()
    }

    /// Keep `g` as window `window_id`'s geometry, dropping the windows `st` no longer has.
    pub(crate) fn set_geometry(&self, st: &State, window_id: &str, g: leon_core::overlays::Geometry) {
        let mut map = events::lock(&self.geometry);
        map.retain(|w, _| st.sessions.iter().any(|s| s.windows.iter().any(|x| &x.id == w)));
        map.insert(window_id.to_string(), g);
    }

    /// The model. A handler that panicked mid-update leaves it usable, so a poisoned lock is taken anyway.
    pub fn state(&self) -> StateGuard<'_> {
        StateGuard { state: events::lock(&self.state), subscribers: &self.subscribers }
    }

    /// Look at each terminal's program in front; a change is a `foreground_changed` event. Off
    /// the state lock, since the event takes it.
    pub fn check_foregrounds(&self) {
        let terminals: Vec<_> =
            self.state().sessions.iter().flat_map(|s| s.blocks.iter().map(|b| b.terminal.clone())).collect();
        terminals.iter().for_each(|t| t.check_foreground());
    }

    pub fn is_autostarted(&self) -> bool {
        self.autostarted.load(Ordering::SeqCst)
    }

    /// End every program before the server exits: hang them up, give them `grace` to finish, then
    /// SIGKILL what is left. Waiting here matters because Drop's delayed kill dies with the process.
    pub async fn end_programs(&self, grace: std::time::Duration) {
        let mut exits = self.exited.subscribe();
        let terminals: Vec<_> = {
            let mut st = self.state();
            // Under the lock: no session is created after this, by a connection still open or otherwise.
            self.stopping.store(true, Ordering::SeqCst);
            let t = st.sessions.iter().flat_map(|s| s.blocks.iter().map(|b| b.terminal.clone())).collect();
            st.sessions.clear();
            t
        };
        terminals.iter().for_each(|t| t.hang_up());
        let all_exited = async {
            while !terminals.iter().all(|t| t.has_exited()) {
                if exits.changed().await.is_err() {
                    return;
                }
            }
        };
        let _ = tokio::time::timeout(grace, all_exited).await;
        terminals.iter().for_each(|t| t.kill());
    }

    /// Run a method. The checks run now; await the result.
    pub fn call(self: &Arc<Self>, method: &str, params: serde_json::Value) -> Later<serde_json::Value> {
        // a call in-process (the app) is the person's; where a close leaves the keys is a side effect
        let moves = focus_guard::asked().unwrap_or(true) && !matches!(method, "block.close" | "session.close_window");
        let later = focus_guard::within(moves, || self.registry.call(self, method, params));
        Box::pin(focus_guard::scope(moves, later))
    }
}
