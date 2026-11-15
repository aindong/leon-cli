//! Keys and pastes into the focused pane, and the link's note for the bar. Over a host's link they
//! go through `typing`, whole or not at all (daily-driver F12).

use leon_app::remote::LinkState;
use ratatui::crossterm::event::KeyEvent;
use serde_json::{json, Value};

use crate::attach::typing::Out;
use crate::attach::{keys, Mode, Msg, Tui};

/// The app's limit for one paste (a request over the server's 1 MiB would drop the connection).
const MAX_PASTE: usize = 400 << 10;

impl Tui {
    /// While the link to the host is retried: which try, for the bar.
    pub(super) fn reconnecting(&self) -> Option<String> {
        match (&self.host, self.remote.link_state()) {
            (Some(h), LinkState::Reconnecting { attempt, .. }) => {
                let lost = self.typing.not_sent().map(|n| format!(", {n}")).unwrap_or_default();
                Some(format!("reconnecting to {h} (try {attempt})…{lost}"))
            }
            _ => None,
        }
    }

    /// Type a key into the focused pane, encoded for its cursor-key mode. Over a link it goes
    /// whole or not at all (`typing`).
    pub(super) fn forward(&mut self, k: &KeyEvent) {
        let Some(t) = self.target() else { return };
        let app_cursor = self.remote.model().screens.get(&t.block).is_some_and(|f| f.app_cursor);
        let Some(bytes) = keys::bytes(k, app_cursor) else { return };
        if !self.remote.is_link() {
            return self.remote.write(&t.sid, &t.block, &bytes);
        }
        let up = self.remote.link_state() == LinkState::Up;
        let next = self.typing.key(&t.sid, &t.block, bytes, up, std::time::Instant::now());
        self.send_typed(next);
    }

    /// Send what `typing` asks for; its answer comes back as `Msg::Typed`. Dropped keys are noted.
    pub(in crate::attach) fn send_typed(&mut self, next: Out) {
        let tx = self.tx.clone();
        let done = move |r: Result<Value, leon_core::ApiError>| {
            let _ = tx.send(Msg::Typed(r.is_ok()));
        };
        match next {
            Out::Nothing => {}
            Out::Probe => self.remote.send_then("server.version", json!({}), done),
            Out::Write(sid, block, bytes) => {
                let args = json!({ "data": String::from_utf8_lossy(&bytes) });
                let p =
                    json!({ "session_id": sid, "block_id": block, "method": "dev.leon.terminal.write", "args": args });
                self.remote.send_then("block.method", p, done);
            }
            Out::Paste(sid, block, text) => {
                let p = json!({ "session_id": sid, "block_id": block, "event": { "type": "paste", "data": text } });
                self.remote.send_then("block.event", p, done);
            }
        }
        if let (Some(n), Some(h)) = (self.typing.not_sent(), &self.host) {
            self.say(format!("{n}: the link to {h} stalled or dropped"));
        }
    }

    pub(in crate::attach) fn paste(&mut self, text: &str) {
        if let Mode::Picker(p, _) = &mut self.mode {
            p.query.push_str(text.lines().next().unwrap_or_default());
            p.selected = 0;
            return;
        }
        let Some(t) = self.target() else { return };
        if text.len() > MAX_PASTE {
            return self.say(format!("not pasting {} bytes: at most {MAX_PASTE} go in one paste", text.len()));
        }
        if self.remote.is_link() {
            let up = self.remote.link_state() == LinkState::Up;
            let next = self.typing.paste(&t.sid, &t.block, text, up, std::time::Instant::now());
            return self.send_typed(next);
        }
        let event = json!({ "session_id": t.sid, "block_id": t.block, "event": { "type": "paste", "data": text } });
        if let Err(e) = self.remote.call("block.event", event) {
            self.say(e.message);
        }
    }
}
