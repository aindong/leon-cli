//! The event connection's reader thread, and the model fetch it triggers (moved as is from
//! remote.rs, which owns the shared state).

use super::*;

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

#[derive(Deserialize)]
struct Pushed {
    block_id: String,
    #[serde(flatten)]
    update: Update,
}

/// Open an event connection and attach it to every event; returns its reader.
pub(super) fn attach(socket: &Path) -> io::Result<(BufReader<UnixStream>, UnixStream)> {
    let mut c = Conn::open(socket, Some(CALL_TIMEOUT))?;
    c.call("session.attach_all", json!({ "event_scope": "all" })).map_err(|e| io::Error::other(e.message))?;
    c.r.get_ref().set_read_timeout(None)?;
    let write = c.r.get_ref().try_clone()?;
    Ok((c.r, write))
}

/// The reader thread: apply screens, refetch after events, re-attach after a drop while the
/// server still answers, and say the server is gone when it doesn't.
pub(super) fn read_events(shared: Arc<Shared>, mut r: BufReader<UnixStream>) {
    loop {
        let mut stale = false;
        loop {
            let mut line = String::new();
            if matches!(read_line(&mut r, &mut line), Ok(0) | Err(_)) {
                if shared.closing.load(Ordering::SeqCst) {
                    return;
                }
                match reattach(&shared) {
                    Some(again) => (r, stale) = (again, true),
                    None => {
                        shared.alive.store(false, Ordering::SeqCst);
                        shared.bump();
                        return;
                    }
                }
                break;
            }
            let Ok(mut v) = serde_json::from_str::<Value>(&line) else { continue };
            if let Some(s) = v.get_mut("screen") {
                // a line still on its way for a block no longer watched would revive a stale copy
                let pushed = serde_json::from_value::<Pushed>(s.take()).ok();
                if let Some(p) = pushed.filter(|p| lock(&shared.watched).contains(&p.block_id)) {
                    let mut m = lock(&shared.model);
                    *m.screen_updates.entry(p.block_id.clone()).or_default() += 1;
                    let f = m.screens.entry(p.block_id).or_default();
                    screen::apply(Arc::make_mut(f), p.update);
                }
            } else if v.get("event").is_some() {
                stale = true;
            } else if let Some(e) = v.get("error").filter(|e| !e.is_null()) {
                eprintln!("leon: the server refused a watch: {e}");
            }
            if !r.buffer().contains(&b'\n') {
                break; // nothing more already read: show what came
            }
        }
        if stale {
            let _ = shared.refresh();
        } else {
            shared.bump();
        }
    }
}

/// A new event connection when the server still answers (it dropped a connection that fell too
/// far behind, or a link to it came back), with the watched blocks watched again.
fn reattach(shared: &Shared) -> Option<BufReader<UnixStream>> {
    let (r, w) = match shared.retry {
        None => {
            lock(&shared.fetch).call("server.version", json!({})).ok()?;
            attach(&shared.socket).ok()?
        }
        Some(limit) => relink(shared, limit)?,
    };
    // held while sending, as `Remote::watch` holds it, so a newer set can't be overtaken
    let watched = lock(&shared.watched);
    *lock(&shared.events) = w;
    if shared.closing.load(Ordering::SeqCst) {
        let _ = lock(&shared.events).shutdown(std::net::Shutdown::Both); // dropped meanwhile
        return None;
    }
    lock(&shared.model).screens.clear(); // the new connection starts with whole screens
    shared.send_watch(&watched).ok()?;
    Some(r)
}

/// Open the fetch and call connections again until the server answers (backing off from 100 ms
/// to 5 s, for up to `limit`), then attach a new event connection.
fn relink(shared: &Shared, limit: Duration) -> Option<(BufReader<UnixStream>, UnixStream)> {
    let end = std::time::Instant::now() + limit;
    let mut wait = Duration::from_millis(100);
    loop {
        let answered = Conn::open(&shared.socket, Some(CALL_TIMEOUT))
            .map_err(unreachable)
            .and_then(|mut c| c.call("server.version", json!({})).map(|_| c));
        if let Ok(fetch) = answered {
            let attached = attach(&shared.socket);
            if let (Ok(attached), Ok(calls)) = (attached, Conn::open(&shared.socket, Some(CALL_TIMEOUT))) {
                *lock(&shared.fetch) = fetch;
                *lock(&shared.calls) = calls;
                return Some(attached);
            }
        }
        if shared.closing.load(Ordering::SeqCst) || std::time::Instant::now() + wait > end {
            return None;
        }
        std::thread::sleep(wait);
        wait = (wait * 2).min(Duration::from_secs(5));
    }
}
