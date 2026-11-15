//! Reading from a server: one line at a time, and the whole model (what `Shared::refresh`
//! replaces the copy with).

use std::io::{self, BufRead, BufReader};
use std::os::unix::net::UnixStream;
use std::time::Instant;

use leon_core::layout::Node;
use leon_core::overlays::Overlays;
use leon_core::program_status::{strip_hidden, Record};
use leon_core::{ApiError, Window};
use serde_json::{json, Value};

use super::{Block, Conn, Session};

/// The longest line read from a server, which may be a box we don't control: the server splits
/// a screen past `leon_server::screens::MAX_SCREEN_LINE` (4 MiB) and caps `lines` replies the same.
/// shortcut: a reply is still parsed into a `Value` (tens of bytes per byte for a hostile one, once
/// per call the app makes); parse replies into their types if remote hosts get untrusted users.
pub(super) const MAX_LINE: u64 = 16 << 20;

/// One line, at most `MAX_LINE` bytes; 0 at the end of the stream. With an `end`, each read gets
/// only what is left until then, so a line trickling in can't outlast it. Bytes that aren't UTF-8
/// are replaced: the line then parses or is skipped like any other bad line.
pub(super) fn read_line(r: &mut BufReader<UnixStream>, out: &mut String, end: Option<Instant>) -> io::Result<usize> {
    let mut bytes = Vec::new();
    loop {
        if r.buffer().is_empty() && end.is_some() {
            r.get_ref().set_read_timeout(super::left(end)?)?;
        }
        let got = match r.fill_buf() {
            Ok(b) => b,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };
        let (n, done) = match got.iter().position(|&b| b == b'\n') {
            Some(i) => (i + 1, true),
            None => (got.len(), got.is_empty()),
        };
        if (bytes.len() + n) as u64 > MAX_LINE {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "a line from the server is too long"));
        }
        bytes.extend_from_slice(&got[..n]);
        r.consume(n);
        if done {
            break;
        }
    }
    let n = bytes.len();
    *out = String::from_utf8(bytes).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned());
    Ok(n)
}

/// The model, read with one call per session and per block (a session that vanishes meanwhile is
/// left out; the event that says so brings another refetch).
/// shortcut: every batch of events refetches every block's status records; fetch only the blocks
/// the events name if a status flood ever shows up in a profile.
pub(super) fn fetch(c: &mut Conn) -> Result<Vec<Session>, ApiError> {
    let list = c.call("session.list", json!({}))?;
    let mut out = Vec::new();
    for s in list["sessions"].as_array().into_iter().flatten().take(MAX_SESSIONS) {
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
            .take(MAX_LISTED)
            .map(|w| Window {
                id: text(&w["window_id"]).unwrap_or_default(),
                label: text(&w["label"]),
                // a ratio outside (0, 1) gives rects that aren't finite
                layout: serde_json::from_value(w["layout"].clone()).ok().filter(ratios_ok),
                focus: text(&w["focused_block_id"]),
                zoomed: text(&w["zoomed_block_id"]),
            })
            .collect();
        let overlays = view["windows"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|w| (text(&w["window_id"]).unwrap_or_default(), decode_overlays(w)))
            .filter(|(_, o)| *o != Overlays::default())
            .collect();
        let active = view["windows"].as_array().and_then(|ws| ws.iter().position(|w| w["active"] == true));
        let mut session = Session {
            id: text(sid).unwrap_or_default(),
            label: text(&view["label"]),
            revision: view["revision"].as_u64().unwrap_or(0),
            active: active.unwrap_or(0),
            windows,
            blocks: Vec::new(),
            overlays,
            tiled_focus: Default::default(),
        };
        for b in blocks["blocks"].as_array().into_iter().flatten().take(MAX_LISTED) {
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
                label: text(&b["label"]),
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

/// Whether every split in `n` has a ratio a server should send (`check_ratio`).
fn ratios_ok(n: &Node) -> bool {
    match n {
        Node::Block { .. } => true,
        Node::Split { ratio, before, after, .. } => {
            leon_core::panes::check_ratio(*ratio).is_ok() && ratios_ok(before) && ratios_ok(after)
        }
    }
}

/// A record's words with nothing hidden (a server we don't run may not have checked them).
fn shown(mut r: Record) -> Record {
    r.title = r.title.map(|t| strip_hidden(&t));
    r.msg = r.msg.map(|m| strip_hidden(&m));
    r
}

/// The most floats a window may have.
pub const MAX_FLOATS: usize = 64;
/// The most sessions read from a server, and windows and blocks per session: the sidebar, the
/// model's fetch and each poll do work per item, and a server may list 40,000 under the line cap.
pub const MAX_SESSIONS: usize = 256;
pub const MAX_LISTED: usize = 1024;

/// A window's floats and drawers from its `session.view` entry. Only the fields this client knows
/// are read (a newer server may add some, and the types refuse unknown ones), and a window whose
/// placements a server should never send (bounds outside the window, a size past 1) gets none.
pub fn decode_overlays(window: &Value) -> Overlays {
    let list = |key: &str| window[key].as_array().map_or(&[][..], Vec::as_slice);
    // each frame does work per float per float: a list past what a person could use is refused
    if list("floating").len() > MAX_FLOATS {
        return Overlays::default();
    }
    // the known fields, the id read as block ids are (nothing hidden)
    let pick = |v: &Value, keys: &[&str]| -> Value {
        let mut out: Value =
            keys.iter().map(|k| (k.to_string(), v[*k].clone())).filter(|(_, v)| !v.is_null()).collect();
        if let Some(id) = v["block_id"].as_str() {
            out["block_id"] = json!(strip_hidden(id));
        }
        out
    };
    let floating: Vec<Value> = list("floating")
        .iter()
        .map(|f| {
            let mut f = pick(f, &["block_id", "bounds", "hidden"]);
            f["bounds"] = pick(&f["bounds"], &["x", "y", "width", "height"]);
            f
        })
        .collect();
    let docked: Vec<Value> = list("docked").iter().map(|d| pick(d, &["block_id", "side", "size", "hidden"])).collect();
    serde_json::from_value::<Overlays>(json!({ "floating": floating, "docked": docked }))
        .ok()
        .filter(|o| o.check().is_ok())
        // each block is placed once
        .filter(|o| o.ids().collect::<std::collections::HashSet<_>>().len() == o.ids().count())
        .unwrap_or_default()
}
