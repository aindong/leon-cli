//! Reading from a server: one line at a time, and the whole model (what `Shared::refresh`
//! replaces the copy with).

use std::io::{self, BufRead, BufReader, Read};
use std::os::unix::net::UnixStream;

use leon_core::program_status::{strip_hidden, Record};
use leon_core::{ApiError, Window};
use serde_json::{json, Value};

use super::{Block, Conn, Session};

/// The longest line read from a server, which may be a box we don't control: the server splits
/// a screen past `leon_server::screens::MAX_SCREEN_LINE` (4 MiB) and caps `lines` replies the same.
/// shortcut: a reply is still parsed into a `Value` (tens of bytes per byte for a hostile one, once
/// per call the app makes); parse replies into their types if remote hosts get untrusted users.
pub(super) const MAX_LINE: u64 = 16 << 20;

/// One line, at most `MAX_LINE` bytes; 0 at the end of the stream.
pub(super) fn read_line(r: &mut BufReader<UnixStream>, out: &mut String) -> io::Result<usize> {
    let mut bytes = Vec::new();
    let n = r.by_ref().take(MAX_LINE + 1).read_until(b'\n', &mut bytes)?;
    if n as u64 > MAX_LINE {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "a line from the server is too long"));
    }
    *out = String::from_utf8(bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Ok(n)
}

/// The model, read with one call per session and per block (a session that vanishes meanwhile is
/// left out; the event that says so brings another refetch).
/// shortcut: every batch of events refetches every block's status records; fetch only the blocks
/// the events name if a status flood ever shows up in a profile.
pub(super) fn fetch(c: &mut Conn) -> Result<Vec<Session>, ApiError> {
    let list = c.call("session.list", json!({}))?;
    let mut out = Vec::new();
    for s in list["sessions"].as_array().into_iter().flatten() {
        let sid = &s["session_id"];
        let (view, blocks) = (
            c.call("session.view", json!({ "session_id": sid })),
            c.call("session.list_blocks", json!({ "session_id": sid })),
        );
        // a connection that broke midway keeps the old model; only a session gone meanwhile is left out
        let (view, blocks) = match (view, blocks) {
            (Ok(v), Ok(b)) => (v, b),
            (Err(e), _) | (_, Err(e)) if e.code == "unreachable" => return Err(e),
            _ => continue,
        };
        // a remote server's names go on screen: nothing that reorders or hides text
        let text = |v: &Value| v.as_str().map(strip_hidden);
        let windows: Vec<Window> = view["windows"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|w| Window {
                id: text(&w["window_id"]).unwrap_or_default(),
                label: text(&w["label"]),
                layout: serde_json::from_value(w["layout"].clone()).ok(),
                focus: text(&w["focused_block_id"]),
                zoomed: text(&w["zoomed_block_id"]),
            })
            .collect();
        let active = view["windows"].as_array().and_then(|ws| ws.iter().position(|w| w["active"] == true));
        let mut session = Session {
            id: text(sid).unwrap_or_default(),
            label: text(&view["label"]),
            revision: view["revision"].as_u64().unwrap_or(0),
            active: active.unwrap_or(0),
            windows,
            blocks: Vec::new(),
        };
        for b in blocks["blocks"].as_array().into_iter().flatten() {
            let p =
                json!({ "session_id": sid, "block_id": b["block_id"], "method": "dev.leon.terminal.program_status" });
            // a broken connection keeps the old model: empty records would read as the status clearing
            let records = match c.call("block.method", p) {
                Err(e) if e.code == "unreachable" => return Err(e),
                r => r.unwrap_or_default(),
            };
            let status = records["records"].as_array().into_iter().flatten();
            session.blocks.push(Block {
                id: text(&b["block_id"]).unwrap_or_default(),
                window_id: text(&b["window_id"]).unwrap_or_default(),
                command: serde_json::from_value(b["command"].clone()).unwrap_or_default(),
                title: text(&b["title"]),
                cwd: text(&b["cwd"]),
                status: status.filter_map(|r| serde_json::from_value(r["record"].clone()).ok()).map(shown).collect(),
            });
        }
        out.push(session);
    }
    Ok(out)
}

/// A record's words with nothing hidden (a server we don't run may not have checked them).
fn shown(mut r: Record) -> Record {
    r.title = r.title.map(|t| strip_hidden(&t));
    r.msg = r.msg.map(|m| strip_hidden(&m));
    r
}
