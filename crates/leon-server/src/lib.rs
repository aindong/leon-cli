//! The leon server: a self-describing method registry over the model, served on a Unix socket.
//! The socket loop (`serve`) takes a `Ctx` the caller built, so the app can embed the same state.

mod blocks;
pub mod events;
mod layout;
pub mod methods;
pub mod registry;
pub mod serve;

use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, Weak};

use leon_core::State;

use events::Subscriber;

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
        }
    }

    /// The model. A handler that panicked mid-update leaves it usable, so a poisoned lock is taken anyway.
    pub fn state(&self) -> StateGuard<'_> {
        StateGuard { state: events::lock(&self.state), subscribers: &self.subscribers }
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
        self.registry.call(self, method, params)
    }
}
