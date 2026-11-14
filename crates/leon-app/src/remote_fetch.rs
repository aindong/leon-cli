//! Reading from a server: one line at a time, and the whole model (what `Shared::refresh`
//! replaces the copy with).

use std::io::{self, BufRead, BufReader, Read};
use std::os::unix::net::UnixStream;

use leon_core::{ApiError, Window};
use serde_json::{json, Value};

use super::{Block, Conn, Session};

/// The longest line read from the server: a whole 500 × 1000 screen of one-cell spans fits.
const MAX_LINE: u64 = 256 << 20;

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
        let (Ok(view), Ok(blocks)) = (
            c.call("session.view", json!({ "session_id": sid })),
            c.call("session.list_blocks", json!({ "session_id": sid })),
        ) else {
            continue;
        };
        let text = |v: &Value| v.as_str().map(String::from);
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
            let records = c.call("block.method", p).unwrap_or_default();
            let status = records["records"].as_array().into_iter().flatten();
            session.blocks.push(Block {
                id: text(&b["block_id"]).unwrap_or_default(),
                window_id: text(&b["window_id"]).unwrap_or_default(),
                command: serde_json::from_value(b["command"].clone()).unwrap_or_default(),
                title: text(&b["title"]),
                cwd: text(&b["cwd"]),
                status: status.filter_map(|r| serde_json::from_value(r["record"].clone()).ok()).collect(),
            });
        }
        out.push(session);
    }
    Ok(out)
}
