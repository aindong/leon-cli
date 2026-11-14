//! What a `Remote` does over a host's link (`connect_retrying`) and not on this machine's socket:
//! the event stream is resumable and a drop is resumed after the last seq seen; a watchdog pings
//! a quiet event connection and ends it when nothing answers (a half-dead link); and the window's
//! calls go through one ordered queue thread, so a slow link never holds the UI thread.

use std::io::{self, BufReader};
use std::os::unix::net::UnixStream;
use std::sync::atomic::Ordering;
use std::sync::mpsc::Receiver;
use std::sync::Weak;
use std::time::{Duration, Instant};

use leon_core::ApiError;
use serde_json::{json, Value};

use super::{attach, lock, unreachable, Conn, Shared, CALL_TIMEOUT};

/// A quiet event connection is pinged after this long, and ended when as long again brings nothing.
const PING_AFTER: Duration = Duration::from_secs(5);

/// The link as the reader last found it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkState {
    Up,
    /// Trying to get the connections back: the try number, and the wait before the next one.
    Reconnecting {
        attempt: u32,
        retry_in: Duration,
    },
}

/// The reply a queued call hands back, on the queue thread.
pub(super) type Done = Box<dyn FnOnce(Result<Value, ApiError>) + Send>;

pub(super) struct Job {
    pub method: String,
    pub params: Value,
    /// Refetch the model after the call (a layout change).
    pub refresh: bool,
    pub done: Option<Done>,
}

/// Run the window's calls in order on the calls connection (no ssh login of its own; a
/// reconnect replaces it). While the link is down a call fails at once: keys typed meanwhile are
/// lost, as in `leon attach -H`.
pub(super) fn run_queue(shared: Weak<Shared>, jobs: Receiver<Job>) {
    for job in jobs {
        let Some(shared) = shared.upgrade() else { return };
        let up = *lock(&shared.link) == LinkState::Up; // not held over the call: the UI reads it
        let r = if up { shared.call(&job.method, job.params) } else { Err(unreachable("the link is down")) };
        if job.refresh {
            let _ = shared.refresh();
        }
        match job.done {
            Some(done) => done(r),
            None => {
                if let Err(e) = r {
                    eprintln!("leon: {} failed: {}", job.method, e.message);
                }
            }
        }
    }
}

/// Every second: ping the event connection once it has been quiet for `PING_AFTER`, and shut it
/// down (the reader then reconnects) when `PING_AFTER` more passes without a line.
pub(super) fn watchdog(shared: Weak<Shared>) {
    loop {
        std::thread::sleep(Duration::from_secs(1));
        let Some(s) = shared.upgrade() else { return };
        if s.closing.load(Ordering::SeqCst) {
            return;
        }
        if *lock(&s.link) != LinkState::Up {
            continue;
        }
        let heard = *lock(&s.heard);
        let mut ping = lock(&s.ping);
        match *ping {
            None if heard.elapsed() >= PING_AFTER => {
                let line = b"{\"id\":0,\"method\":\"server.version\",\"params\":{}}\n";
                let _ = io::Write::write_all(&mut *lock(&s.events), line);
                *ping = Some(Instant::now());
            }
            Some(at) if at.elapsed() >= PING_AFTER && heard < at => {
                let _ = lock(&s.events).shutdown(std::net::Shutdown::Both);
                *ping = None;
            }
            _ => {}
        }
    }
}

/// Open the fetch and call connections again until the server answers (backing off from 100 ms
/// to 5 s, for up to `limit`), then resume the event stream after the last seq seen, or, when the
/// server can't (it restarted), attach again.
pub(super) fn relink(shared: &Shared, limit: Duration) -> Option<(BufReader<UnixStream>, UnixStream)> {
    let end = Instant::now() + limit;
    let mut wait = Duration::from_millis(100);
    for attempt in 1.. {
        *lock(&shared.link) = LinkState::Reconnecting { attempt, retry_in: wait };
        shared.bump();
        let answered = Conn::open(&shared.socket, Some(CALL_TIMEOUT))
            .map_err(unreachable)
            .and_then(|mut c| c.call("server.version", json!({})).map(|_| c));
        if let Ok(fetch) = answered {
            if let (Some(events), Ok(calls)) =
                (resume_or_attach(shared), Conn::open(&shared.socket, Some(CALL_TIMEOUT)))
            {
                *lock(&shared.fetch) = fetch;
                *lock(&shared.calls) = calls;
                *lock(&shared.heard) = Instant::now();
                *lock(&shared.ping) = None;
                *lock(&shared.link) = LinkState::Up;
                return Some(events);
            }
        }
        if shared.closing.load(Ordering::SeqCst) || Instant::now() + wait > end {
            return None;
        }
        std::thread::sleep(wait);
        wait = (wait * 2).min(Duration::from_secs(5));
    }
    None
}

/// A new event connection carrying the old stream from the seq after the last one seen.
fn resume_or_attach(shared: &Shared) -> Option<(BufReader<UnixStream>, UnixStream)> {
    let token = lock(&shared.resume).clone();
    if let Some(token) = token {
        let mut c = Conn::open(&shared.socket, Some(CALL_TIMEOUT)).ok()?;
        let seq = shared.seq.load(Ordering::SeqCst);
        match c.call("session.resume", json!({ "resume": token, "seq": seq })) {
            Ok(_) => {
                c.r.get_ref().set_read_timeout(None).ok()?;
                let write = c.r.get_ref().try_clone().ok()?;
                shared.resumes.fetch_add(1, Ordering::SeqCst);
                return Some((c.r, write));
            }
            Err(e) if e.code != "resume_failed" => return None,
            Err(_) => {}
        }
    }
    let (r, w, snapshot) = attach(&shared.socket, true, CALL_TIMEOUT).ok()?;
    shared.reattaches.fetch_add(1, Ordering::SeqCst);
    shared.restart_stream(&snapshot);
    Some((r, w))
}
