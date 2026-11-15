//! Event subscriptions. A connection that attaches gets a `Subscriber`: what it asked for, its own
//! gapless seq, and a queue of encoded lines its socket task writes out. Events are queued while
//! the state lock is held and the snapshot is taken under that same lock, so a client applies the
//! snapshot, then every event with a higher seq, and misses nothing.
//!
//! A subscriber whose queue would pass `MAX_BACKLOG` is dropped and its queue freed (docs/api.md). Queueing never waits, so a client that stops reading slows nobody else. A resumable one
//! also keeps the lines it wrote last, for `session.resume` (resume.rs).

use std::collections::{HashSet, VecDeque};
use std::sync::{Arc, Mutex, MutexGuard};

use leon_core::events::Event;
use leon_core::ApiError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::Notify;

use crate::methods::{session_list, view, SessionSummary, SessionView};
use crate::registry::parse;
use crate::{Ctx, Registry};

/// How far behind (in encoded bytes) a connection may fall before it is dropped.
pub const MAX_BACKLOG: usize = 8 << 20;

pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

#[derive(Deserialize, JsonSchema, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EventScope {
    /// Session lifecycle: `session_created`, `session_destroyed`.
    Session,
    /// Everything, in every session.
    All,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AttachAll {
    pub event_scope: EventScope,
    /// Also return a `resume` token: when this connection drops, `session.resume` on another one
    /// carries on with the next seq.
    #[serde(default)]
    pub resumable: bool,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AttachSession {
    pub session_id: String,
    /// As in `session.attach_all`.
    #[serde(default)]
    pub resumable: bool,
}

#[derive(Serialize, JsonSchema)]
pub struct Attached {
    /// The connection's event seq at this snapshot; every later event on it has a higher one.
    pub seq: u64,
    pub view: SessionView,
    /// With `resumable`: the token `session.resume` takes, good for every later drop of this stream.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resume: Option<String>,
}

#[derive(Serialize, JsonSchema)]
pub struct AttachedAll {
    /// As in `session.attach`.
    pub seq: u64,
    pub sessions: Vec<SessionSummary>,
    /// As in `session.attach`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resume: Option<String>,
}

#[derive(Default)]
pub(crate) struct Queue {
    pub(crate) all: bool,
    pub(crate) lifecycle: bool,
    pub(crate) sessions: HashSet<String>,
    pub(crate) seq: u64,
    /// Encoded lines not yet written, with their seq.
    pub(crate) lines: VecDeque<(u64, Vec<u8>)>,
    pub(crate) bytes: usize,
    pub(crate) dropped: bool,
    pub(crate) registered: bool,
    /// A resumable stream's token.
    pub(crate) resume: Option<String>,
    /// A resumable stream's last written lines (up to `resume::KEEP` bytes): taken off `lines`
    /// for writing, so a line whose write failed, or that the client never read, is here.
    pub(crate) kept: VecDeque<(u64, Vec<u8>)>,
    pub(crate) kept_bytes: usize,
}

impl Queue {
    fn wants(&self, e: &Event) -> bool {
        self.all || (e.server && self.lifecycle) || self.sessions.contains(&e.session_id)
    }
}

/// The subscriber was dropped for falling too far behind.
#[derive(Debug)]
pub struct Dropped;

#[derive(Default)]
pub struct Subscriber {
    pub(crate) queue: Mutex<Queue>,
    /// Woken when a line is queued or the subscriber is dropped.
    pub ready: Notify,
}

impl Subscriber {
    /// Queue `e` as `{"event":…,"seq":N}` if it is in scope. Called with the state lock held.
    pub(crate) fn offer(&self, e: &Event) {
        let mut q = lock(&self.queue);
        if q.dropped || !q.wants(e) {
            return;
        }
        q.seq += 1;
        let mut line = serde_json::to_vec(&json!({ "event": e.body, "seq": q.seq })).expect("events serialize");
        line.push(b'\n');
        q.bytes += line.len();
        if q.bytes > MAX_BACKLOG {
            q.dropped = true;
            q.lines = VecDeque::new();
        } else {
            let seq = q.seq;
            q.lines.push_back((seq, line));
        }
        drop(q);
        self.ready.notify_one();
    }

    /// The next queued line, if any.
    pub fn pop(&self) -> Result<Option<Vec<u8>>, Dropped> {
        let mut q = lock(&self.queue);
        if q.dropped {
            return Err(Dropped);
        }
        let Some((seq, line)) = q.lines.pop_front() else { return Ok(None) };
        q.bytes -= line.len();
        if q.resume.is_some() {
            q.kept_bytes += line.len();
            q.kept.push_back((seq, line.clone()));
            while q.kept_bytes > crate::resume::KEEP {
                let (_, old) = q.kept.pop_front().expect("kept bytes are in kept lines");
                q.kept_bytes -= old.len();
            }
        }
        Ok(Some(line))
    }

    pub fn has_lines(&self) -> bool {
        !lock(&self.queue).lines.is_empty()
    }

    pub fn is_dropped(&self) -> bool {
        lock(&self.queue).dropped
    }
}

impl Ctx {
    /// Route an attach call from a connection to `sub`; the socket loop uses this instead of the registry.
    pub fn attach(&self, sub: &Arc<Subscriber>, method: &str, params: Value) -> Result<Value, ApiError> {
        let r = match method {
            "session.attach" => serde_json::to_value(self.attach_session(sub, parse(method, params)?)?),
            "session.resume" => serde_json::to_value(self.resume(sub, parse(method, params)?)?),
            _ => serde_json::to_value(self.attach_all(sub, parse(method, params)?)?),
        };
        Ok(r.expect("snapshots serialize"))
    }

    /// Snapshot one session, and have `sub` get its events from here on.
    pub fn attach_session(&self, sub: &Arc<Subscriber>, p: AttachSession) -> Result<Attached, ApiError> {
        let st = self.state();
        let view = view(st.session(&p.session_id)?);
        let seq = self.widen(sub, |q| {
            q.sessions.insert(p.session_id);
        });
        let resume = p.resumable.then(|| self.make_resumable(sub));
        Ok(Attached { seq, view, resume })
    }

    /// List the sessions, and have `sub` get the events of `event_scope` from here on.
    pub fn attach_all(&self, sub: &Arc<Subscriber>, p: AttachAll) -> Result<AttachedAll, ApiError> {
        let st = self.state();
        let sessions = session_list(&st);
        let seq = self.widen(sub, |q| match p.event_scope {
            EventScope::Session => q.lifecycle = true,
            EventScope::All => q.all = true,
        });
        let resume = p.resumable.then(|| self.make_resumable(sub));
        Ok(AttachedAll { seq, sessions, resume })
    }

    /// Change what `sub` gets and register it. Call with the state lock held. Returns its seq.
    fn widen(&self, sub: &Arc<Subscriber>, f: impl FnOnce(&mut Queue)) -> u64 {
        let mut q = lock(&sub.queue);
        f(&mut q);
        let (seq, first) = (q.seq, !std::mem::replace(&mut q.registered, true));
        drop(q);
        if first {
            self.register(sub);
        }
        seq
    }

    /// Have `sub` get events. Call with the state lock held.
    pub(crate) fn register(&self, sub: &Arc<Subscriber>) {
        let mut subs = lock(&self.subscribers);
        subs.retain(|s| s.strong_count() > 0);
        subs.push(Arc::downgrade(sub));
    }
}

/// Whether the socket loop must route `method` through `Ctx::attach`.
pub fn is_attach(method: &str) -> bool {
    matches!(method, "session.attach" | "session.attach_all" | "session.resume")
}

pub(crate) fn add(r: &mut Registry) {
    // A connection's own calls go through `Ctx::attach` (serve.rs); these serve help.all and in-process callers.
    r.add(
        "session.attach",
        "Snapshot a session; on a socket connection, also receive its events (block events too) as \
         `{\"event\":…,\"seq\":N}` lines from now on, with seq above the snapshot's.",
        |ctx, p: AttachSession| ctx.attach_session(&Arc::default(), p),
    );
    r.add(
        "session.attach_all",
        "List sessions; on a socket connection, also receive session lifecycle events (event_scope \
         `session`) or every event (`all`) from now on.",
        |ctx, p: AttachAll| ctx.attach_all(&Arc::default(), p),
    );
}
