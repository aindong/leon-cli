//! The colours a server resolves this app's screens and lines with (`block.watch`'s and
//! `terminal.lines`' `palette`), and what happens when a server too old for them refuses one.

use super::*;

/// A server's refusal of the `palette` field: one older than M22b, which denies unknown fields.
pub(super) fn refuses_palette(message: &str) -> bool {
    message.contains("unknown field `palette`")
}

impl Shared {
    /// The palette to send, unless the server refused one.
    pub(super) fn palette(&self) -> Option<Value> {
        lock(&self.palette).clone().filter(|_| !self.palette_refused.load(Ordering::SeqCst))
    }

    /// The server didn't take a palette: stop sending it, and watch again without (held as
    /// `Remote::watch` holds it).
    pub(super) fn refuse_palette(&self) {
        if !self.palette_refused.swap(true, Ordering::SeqCst) {
            eprintln!("leon: this server is older than leon's themes; its panes keep leon-dark's colours");
            let watched = lock(&self.watched);
            let _ = self.send_watch(&watched);
        }
    }
}

impl Remote {
    /// Resolve this app's screens with `theme`'s terminal colours from now on: the watched screens
    /// come again, whole, in them. Nothing is sent when they didn't change.
    pub fn set_palette(&self, theme: &leon_core::theme::Theme) {
        let p = json!({ "fg": theme.fg, "bg": theme.bg, "cursor": theme.cursor, "ansi": theme.ansi });
        let watched = lock(&self.shared.watched);
        if lock(&self.shared.palette).replace(p.clone()) == Some(p)
            || self.shared.palette_refused.load(Ordering::SeqCst)
        {
            return;
        }
        if let Err(e) = self.shared.send_watch(&watched) {
            eprintln!("leon: can't watch the panes: {e}");
        }
    }

    /// `block.method` `dev.leon.terminal.lines` in this app's palette (without one on a server
    /// that refuses it).
    pub fn lines(&self, session_id: &str, block_id: &str, from: usize, count: usize) -> Result<Value, ApiError> {
        let call = |palette: Option<Value>| {
            let mut args = json!({ "from": from, "count": count });
            if let Some(p) = palette {
                args["palette"] = p;
            }
            let p = json!({ "session_id": session_id, "block_id": block_id, "method": "dev.leon.terminal.lines", "args": args });
            self.call("block.method", p)
        };
        match self.shared.palette() {
            Some(p) => match call(Some(p)) {
                Err(e) if refuses_palette(&e.message) => {
                    self.shared.refuse_palette();
                    call(None)
                }
                r => r,
            },
            None => call(None),
        }
    }
}
