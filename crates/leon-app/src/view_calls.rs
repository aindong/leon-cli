//! The calls a pane makes to its server: they go through the remote's ordered queue (This Mac's
//! too, ADR R8) and the reply is applied on the next wake, so a slow, stuck or hostile server
//! never holds the window (docs/architecture/state-and-hosts.md).

use super::*;

/// A reply to a queued call, applied to the workspace on the next wake.
pub(super) type Reply = Box<dyn FnOnce(&mut Workspace) + Send>;
/// A call waiting for the one of its kind that is out.
pub(super) type Waiting = Box<dyn FnOnce(&mut Workspace)>;

/// A call's reply on its way to the poll. One dropped unanswered (its link went away with the call
/// still queued) answers "unreachable", so nothing waits for it for ever (`term_call_latest`).
struct OnReply {
    replies: Arc<Mutex<Vec<Reply>>>,
    wake: Option<Arc<dyn Fn() + Send + Sync>>,
    then: Option<ThenReply>,
}

/// A call's reply as a task awaits it (`term_ask`).
pub(super) type Answer = Result<Value, leon_core::ApiError>;

type ThenReply = Box<dyn FnOnce(&mut Workspace, Result<Value, leon_core::ApiError>) + Send>;

impl OnReply {
    fn deliver(mut self, r: Result<Value, leon_core::ApiError>) {
        let Some(then) = self.then.take() else { return };
        self.replies.lock().unwrap_or_else(|e| e.into_inner()).push(Box::new(move |ws: &mut Workspace| then(ws, r)));
        if let Some(wake) = &self.wake {
            wake();
        }
    }
}

impl Drop for OnReply {
    fn drop(&mut self) {
        if self.then.is_some() {
            let gone = leon_core::ApiError::new(503, "unreachable", "the link went away");
            OnReply { replies: self.replies.clone(), wake: self.wake.take(), then: self.then.take() }
                .deliver(Err(gone));
        }
    }
}
impl Workspace {
    /// Send an input event (mouse, wheel, paste) to block `id`; over a link it queues like a key.
    pub(super) fn block_event(&self, id: &str, event: Value) {
        let p = json!({ "session_id": self.session_of(id), "block_id": id, "event": event });
        self.app.send("block.event", p);
    }

    fn term_params(&self, id: &str, method: &str, args: Value) -> Value {
        let sid = self.session_of(id);
        json!({ "session_id": sid, "block_id": id, "method": format!("dev.leon.terminal.{method}"), "args": args })
    }

    /// Call a terminal method on block `id` through the queue, for a task to await the reply (one
    /// that needs the window when it comes, as a link's confirmation prompt does).
    pub(super) fn term_ask(&self, id: &str, method: &str, args: Value) -> futures::channel::oneshot::Receiver<Answer> {
        let (tx, rx) = futures::channel::oneshot::channel();
        self.app.send_then("block.method", self.term_params(id, method, args), move |r| drop(tx.send(r)));
        rx
    }

    /// Call a terminal method on block `id`, then `then` with the reply once the queue has run it
    /// (the next wake applies it).
    pub(super) fn term_call_then(
        &mut self,
        id: &str,
        method: &str,
        args: Value,
        then: impl FnOnce(&mut Self, Result<Value, leon_core::ApiError>) + Send + 'static,
    ) {
        let reply =
            OnReply { replies: self.term.replies.clone(), wake: self.term.wake.clone(), then: Some(Box::new(then)) };
        self.app.send_then("block.method", self.term_params(id, method, args), move |r| reply.deliver(r));
    }

    /// `act_later` of `method` on the selected machine, then `then` once its reply came (on the
    /// next wake, after the refetch the call made).
    pub(super) fn act_then(&self, method: &str, params: Value, then: impl FnOnce(&mut Workspace) + Send + 'static) {
        let then: ThenReply = Box::new(move |ws, _| then(ws));
        let reply = OnReply { replies: self.term.replies.clone(), wake: self.term.wake.clone(), then: Some(then) };
        self.app.act_later(method, params, move |r| reply.deliver(r));
    }

    /// `term_call_then` with one call of kind `key` out at a time: a newer one waits for the reply
    /// (replacing any older one waiting), so a stalled server holds one call per kind, not one
    /// per mouse move or key.
    pub(super) fn term_call_latest(
        &mut self,
        key: &str,
        id: &str,
        method: &'static str,
        args: Value,
        then: impl FnOnce(&mut Self, Result<Value, leon_core::ApiError>) + Send + 'static,
    ) {
        if self.term.in_flight.contains(key) {
            let (k, id) = (key.to_string(), id.to_string());
            let next = move |ws: &mut Workspace| ws.term_call_latest(&k, &id, method, args, then);
            self.term.waiting.insert(key.to_string(), Box::new(next));
            return;
        }
        self.term.in_flight.insert(key.to_string());
        let key = key.to_string();
        self.term_call_then(id, method, args, move |ws, r| {
            ws.term.in_flight.remove(&key);
            then(ws, r);
            if let Some(next) = ws.term.waiting.remove(&key) {
                next(ws);
            }
        });
    }

    /// On a wake: apply the replies that came (true when any did), and forget the scroll state of
    /// panes no machine has any more (closed, or their host went).
    pub(super) fn poll_terms(&mut self) -> bool {
        let replies = std::mem::take(&mut *self.term.replies.lock().unwrap_or_else(|e| e.into_inner()));
        let replied = !replies.is_empty();
        for r in replies {
            r(self);
        }
        let hosts = &self.hosts;
        self.term.panes.retain(|id, _| hosts.remotes().any(|(_, r)| r.model().block(id).is_some()));
        replied
    }
}

impl Workspace {
    /// Run a layout method on the selected session; redraw.
    pub(super) fn call(&mut self, method: &str, mut params: serde_json::Value, cx: &mut Context<Self>) {
        let Some(sid) = self.session_id.clone() else { return };
        params["session_id"] = json!(sid);
        self.app.act_later(method, params, drop);
        self.called(cx);
    }

    /// After a layout call was queued: the selection settles on what the model has now (the
    /// refetch after the call brings the change), and the root draws.
    pub(super) fn called(&mut self, cx: &mut Context<Self>) {
        let rows = self.rows();
        self.settle(&rows);
        cx.notify();
    }
}
