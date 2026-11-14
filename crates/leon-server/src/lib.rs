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

    /// Run a method. The checks run now; await the result.
    pub fn call(self: &Arc<Self>, method: &str, params: serde_json::Value) -> Later<serde_json::Value> {
        self.registry.call(self, method, params)
    }
}
