//! An in-process leon server for tests and benches: a `Ctx` served on a socket from a background
//! tokio runtime. The app itself never embeds one; it attaches to a server process over the
//! socket (`remote`), so its programs outlive it.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use leon_core::ApiError;
use leon_server::serve::{self, Options};
use leon_server::{Ctx, Info};
use serde_json::Value;

pub struct Embedded {
    ctx: Arc<Ctx>,
    rt: tokio::runtime::Runtime,
    socket: PathBuf,
    /// Set once the socket loop has ended (stopped by us or by `leon server stop`).
    done: Arc<AtomicBool>,
}

/// Serve on `socket`, keeping no sessions across runs and loading no config.
pub fn start_at(socket: PathBuf) -> Result<Embedded, String> {
    start_with(socket, None, Vec::new())
}

/// Serve on `socket` with `config` as init.lua, keeping no sessions across runs.
pub fn start_configured(socket: PathBuf, config: PathBuf) -> Result<Embedded, String> {
    start_with(socket, Some(config), Vec::new())
}

/// Serve on `socket`, loading `config` (a missing file is an empty config), with `pane_env` set
/// for every pane (`Ctx::pane_env`). Fails if a live server already owns the socket.
pub fn start_with(
    socket: PathBuf,
    config: Option<PathBuf>,
    pane_env: Vec<(String, String)>,
) -> Result<Embedded, String> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .thread_name("leon-server")
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    let listener = rt.block_on(serve::listen(&socket)).map_err(|e| format!("can't serve {}: {e}", socket.display()))?;
    let info =
        Info { pid: std::process::id(), version: env!("CARGO_PKG_VERSION"), socket: socket.display().to_string() };
    let mut ctx = Ctx::new(info, false);
    ctx.pane_env = pane_env;
    let ctx = Arc::new(ctx);
    let done = Arc::new(AtomicBool::new(false));
    if let Some(path) = config {
        let _in_rt = rt.enter(); // the load runs on the runtime while the socket serves
        ctx.start_config(Some(path));
    }
    let opts = Options { socket: socket.clone(), idle_exit: Duration::from_secs(10) };
    let (c, d) = (ctx.clone(), done.clone());
    rt.spawn(async move {
        let _ = serve::serve_on(c, listener, opts).await;
        d.store(true, Ordering::SeqCst);
    });
    Ok(Embedded { ctx, rt, socket, done })
}

impl Embedded {
    pub fn ctx(&self) -> &Arc<Ctx> {
        &self.ctx
    }

    pub fn socket(&self) -> &Path {
        &self.socket
    }

    /// Run a method in-process. Only for methods that answer at once (not `block.wait`).
    pub fn call(&self, method: &str, params: Value) -> Result<Value, ApiError> {
        self.rt.block_on(self.ctx.call(method, params))
    }

    pub fn stopped(&self) -> bool {
        self.done.load(Ordering::SeqCst)
    }

    /// Stop the way `leon server stop` does (programs end, the socket goes), and wait for it.
    pub fn stop(&self) {
        self.ctx.shutdown.notify_one();
        let end = Instant::now() + Duration::from_secs(5);
        while !self.stopped() && Instant::now() < end {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
