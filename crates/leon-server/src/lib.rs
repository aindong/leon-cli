//! The leon server: a self-describing method registry over the model, served on a Unix socket.
//! The socket loop (`serve`) takes a `Ctx` the caller built, so the app can embed the same state.

mod blocks;
pub mod methods;
pub mod registry;
pub mod serve;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use leon_core::State;

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
}

impl Ctx {
    pub fn new(info: Info, autostarted: bool) -> Self {
        Ctx {
            state: Mutex::default(),
            info,
            registry: methods::registry(),
            autostarted: AtomicBool::new(autostarted),
            exited: tokio::sync::watch::channel(0).0,
        }
    }

    /// The model. A handler that panicked mid-update leaves it usable, so a poisoned lock is taken anyway.
    pub fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
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
