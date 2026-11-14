//! The leon server: a self-describing method registry over the model, served on a Unix socket.

pub mod methods;
pub mod registry;
pub mod serve;

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use leon_core::State;

pub use registry::Registry;

/// Facts about this server process, reported by `server.status`.
#[derive(Debug, Clone)]
pub struct Info {
    pub pid: u32,
    pub version: &'static str,
    pub socket: String,
    pub autostarted: bool,
}

/// Everything a method handler can reach.
pub struct Ctx {
    pub state: Mutex<State>,
    pub info: Info,
    pub registry: Arc<Registry>,
    /// Set by `server.stop`; the connection that asked shuts the server down after replying.
    pub stopping: AtomicBool,
}

impl Ctx {
    pub fn new(info: Info) -> Self {
        Ctx { state: Mutex::default(), info, registry: Arc::new(methods::registry()), stopping: AtomicBool::new(false) }
    }

    pub fn call(&self, method: &str, params: serde_json::Value) -> Result<serde_json::Value, leon_core::ApiError> {
        self.registry.call(self, method, params)
    }
}
