//! The event connection: its lines parsed into types, attaching, the reader thread that applies
//! screens and refetches after events, and re-attaching after a drop.

use super::*;

/// A line on the event connection, read into types rather than a `Value` tree: a server (maybe a
/// box we don't control) gets about a byte of memory per byte it sends, not dozens. Unknown and
/// unused fields are skipped without being kept.
#[derive(Deserialize)]
struct Incoming {
    screen: Option<Pushed>,
    event: Option<serde::de::IgnoredAny>,
    seq: Option<u64>,
    error: Option<Refusal>,
}

#[derive(Deserialize)]
struct Refusal {
    message: Option<String>,
}

/// A screen line's body: `Update`'s fields spelled out, since serde's `flatten` would first buffer
/// the whole line as a tree.
#[derive(Deserialize)]
struct Pushed {
    block_id: String,
    full: bool,
    lines: usize,
    cols: usize,
    rows: Vec<screen::Row>,
    cursor: Option<screen::Cursor>,
    #[serde(default)]
    app_cursor: bool,
    #[serde(default)]
    bracketed_paste: bool,
    #[serde(default)]
    scrolled: usize,
    #[serde(default)]
    history: usize,
    #[serde(default)]
    alt_screen: bool,
    #[serde(default)]
    alternate_scroll: bool,
    #[serde(default)]
    mouse: Option<leon_core::mouse::Report>,
}

impl Pushed {
    fn update(self) -> Update {
        let view = screen::View {
            scrolled: self.scrolled,
            history: self.history,
            alt_screen: self.alt_screen,
            alternate_scroll: self.alternate_scroll,
            mouse: self.mouse,
        };
        let (full, lines, cols, rows, cursor) = (self.full, self.lines, self.cols, self.rows, self.cursor);
        Update {
            full,
            lines,
            cols,
            rows,
            cursor,
            app_cursor: self.app_cursor,
            bracketed_paste: self.bracketed_paste,
            view,
        }
    }
}

/// Open an event connection and attach it to every event; returns its reader, its write side
/// and the snapshot (with a `resume` token when `resumable`).
pub(super) fn attach(
    socket: &Path,
    resumable: bool,
    wait: Duration,
) -> io::Result<(BufReader<UnixStream>, UnixStream, Value)> {
    let mut c = Conn::open(socket, Some(wait))?;
    let p = json!({ "event_scope": "all", "resumable": resumable });
    let snapshot = c.call("session.attach_all", p).map_err(|e| io::Error::other(e.message))?;
    c.r.get_ref().set_read_timeout(None)?;
    let write = c.r.get_ref().try_clone()?;
    Ok((c.r, write, snapshot))
}

/// The reader thread: apply screens, refetch after events, re-attach after a drop while the
/// server still answers, and say the server is gone when it doesn't.
pub(super) fn read_events(shared: Arc<Shared>, mut r: BufReader<UnixStream>) {
    loop {
        let mut stale = false;
        loop {
            let mut line = String::new();
            let read = read_line(&mut r, &mut line);
            if matches!(read, Ok(0) | Err(_)) {
                if shared.closing.load(Ordering::SeqCst) {
                    return;
                }
                // a line past the cap would come again after a re-attach: give up on this server
                let refused = matches!(&read, Err(e) if e.kind() == io::ErrorKind::InvalidData);
                if refused {
                    eprintln!("leon: the server sent a line past {} MiB; disconnecting", fetch_model::MAX_LINE >> 20);
                }
                match reattach(&shared).filter(|_| !refused) {
                    Some(again) => (r, stale) = (again, true),
                    None => {
                        shared.alive.store(false, Ordering::SeqCst);
                        shared.bump();
                        return;
                    }
                }
                break;
            }
            *lock(&shared.heard) = Instant::now();
            let Ok(v) = serde_json::from_str::<Incoming>(&line) else { continue };
            if let Some(p) = v.screen {
                // a line still on its way for a block no longer watched would revive a stale copy
                if lock(&shared.watched).contains(&p.block_id) {
                    let mut m = lock(&shared.model);
                    *m.screen_updates.entry(p.block_id.clone()).or_default() += 1;
                    let f = m.screens.entry(p.block_id.clone()).or_default();
                    screen::apply(Arc::make_mut(f), p.update());
                }
            } else if v.event.is_some() {
                if let Some(seq) = v.seq {
                    shared.saw(seq);
                }
                stale = true;
            } else if let Some(e) = v.error {
                eprintln!("leon: the server refused a watch: {}", e.message.unwrap_or_default());
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
            let (r, w, snapshot) = attach(&shared.socket, false, CALL_TIMEOUT).ok()?;
            shared.restart_stream(&snapshot);
            (r, w)
        }
        Some(limit) => link::relink(shared, limit)?,
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
