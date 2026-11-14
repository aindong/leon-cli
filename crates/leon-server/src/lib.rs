//! The leon server: a self-describing method registry over the model, served on a Unix socket.
//! The socket loop (`serve`) takes a `Ctx` the caller built, so the app can embed the same state.

pub mod methods;
pub mod registry;
pub mod serve;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};

use leon_core::State;

pub use registry::Registry;

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
}

impl Ctx {
    pub fn new(info: Info, autostarted: bool) -> Self {
        Ctx { state: Mutex::default(), info, registry: methods::registry(), autostarted: AtomicBool::new(autostarted) }
    }

    /// The model. A handler that panicked mid-update leaves it usable, so a poisoned lock is taken anyway.
    pub fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn is_autostarted(&self) -> bool {
        self.autostarted.load(Ordering::SeqCst)
    }

    pub fn call(&self, method: &str, params: serde_json::Value) -> Result<serde_json::Value, leon_core::ApiError> {
        self.registry.call(self, method, params)
    }
}
