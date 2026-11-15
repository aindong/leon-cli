//! Attached: keys to the focused pane or the prefix table, the actions, and drawing the active tab
//! (its panes, their borders, the status bar).

use leon_app::keymap::{Bound, Pressed};
use leon_app::panes;
use leon_app::remote::LinkState;
use leon_lua::keys::Chord;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::{Position, Rect};
use serde_json::{json, Value};

use super::draw::{self, Bar};
use super::keys::{self, After};
use super::picker::{self, Picker};
use super::{Flow, Mode, Tui};

/// The app's limit for one paste (a request over the server's 1 MiB would drop the connection).
const MAX_PASTE: usize = 400 << 10;

/// The tab being shown: its session, the block keys go to, and whether it is zoomed.
struct Target {
    sid: String,
    block: String,
    zoomed: bool,
}

impl Tui {
    /// While the link to the host is retried: which try, for the bar.
    fn reconnecting(&self) -> Option<String> {
        match (&self.host, self.remote.link_state()) {
            (Some(h), LinkState::Reconnecting { attempt, .. }) => Some(format!("reconnecting to {h} (try {attempt})…")),
            _ => None,
        }
    }

    fn session_id(&self) -> Option<&str> {
        match &self.mode {
            Mode::Attached(sid) => Some(sid),
            _ => None,
        }
    }

    fn target(&self) -> Option<Target> {
        let Mode::Attached(sid) = &self.mode else { return None };
        let m = self.remote.model();
        let w = m.session(sid)?.windows.get(m.session(sid)?.active)?;
        let block = super::overlays::focused(w)?.to_string();
        let zoomed = w.zoomed.as_deref() == Some(block.as_str());
        Some(Target { sid: sid.clone(), block, zoomed })
    }

    pub(super) fn attached_key(&mut self, k: KeyEvent, dirty: &mut bool) -> Flow {
        if super::lock(&self.note).take().is_some() {
            *dirty = true;
        }
        // every key closes the repeat window; only a repeatable one opens it again
        let mut window = std::mem::take(&mut self.repeat);
        let Some(c) = keys::chord(&k) else {
            // a key binding syntax can't name ends any prefix or sequence, and goes to the pane
            *dirty |= self.after_prefix || !self.pending.is_empty();
            (self.after_prefix, self.pending) = (false, Vec::new());
            self.forward(&k);
            return Flow::Go;
        };
        let now = std::time::Instant::now();
        let repeat = self.keys.config.repeat_time;
        if !self.after_prefix && self.pending.is_empty() {
            let keys = &self.keys;
            let find = |p: &[Chord], c: &Chord| match p == std::slice::from_ref(&keys.prefix) {
                true => keys.repeatable(c),
                false => keys.config.repeatable(p, c).map(|b| After::Config(Pressed::Run(b))),
            };
            if let Some((after, prefix)) = window.press(&c, now, find) {
                self.repeat.ran(&prefix, true, repeat, now);
                return match after {
                    After::Run(a, args) => self.action(&a, args.as_ref()),
                    After::Config(Pressed::Run(b)) => self.binding(b, prefix, &c),
                    _ => Flow::Go,
                };
            }
        }
        if self.after_prefix {
            (self.after_prefix, *dirty) = (false, true);
            let prefix = self.keys.prefix.clone();
            let repeats = self.keys.repeatable(&c).is_some();
            self.repeat.ran(std::slice::from_ref(&prefix), repeats, repeat, now);
            return match self.keys.after_prefix(&c) {
                After::Run(a, args) => self.action(&a, args.as_ref()),
                After::Config(Pressed::Run(b)) => self.binding(b, vec![prefix], &c),
                After::Config(_) => {
                    self.pending = vec![prefix, c];
                    Flow::Go
                }
                After::Literal => {
                    self.forward(&k);
                    Flow::Go
                }
                After::Nothing => Flow::Go,
            };
        }
        if self.pending.is_empty() && c == self.keys.prefix {
            (self.after_prefix, *dirty) = (true, true);
            return Flow::Go;
        }
        match self.keys.config.press(&self.pending, &c) {
            Pressed::Run(b) => {
                let prefix = std::mem::take(&mut self.pending);
                self.repeat.ran(&prefix, b.repeats, repeat, now);
                self.binding(b, prefix, &c)
            }
            Pressed::Pending => {
                self.pending.push(c);
                Flow::Go
            }
            Pressed::Unbound => {
                if self.pending.is_empty() {
                    self.forward(&k);
                }
                self.pending.clear();
                Flow::Go
            }
        }
    }

    /// Type a key into the focused pane, encoded for its cursor-key mode.
    fn forward(&self, k: &KeyEvent) {
        let Some(t) = self.target() else { return };
        let app_cursor = self.remote.model().screens.get(&t.block).is_some_and(|f| f.app_cursor);
        if let Some(bytes) = keys::bytes(k, app_cursor) {
            self.remote.write(&t.sid, &t.block, &bytes);
        }
    }

    pub(super) fn paste(&mut self, text: &str) {
        if let Mode::Picker(p, _) = &mut self.mode {
            p.query.push_str(text.lines().next().unwrap_or_default());
            p.selected = 0;
            return;
        }
        let Some(t) = self.target() else { return };
        if text.len() > MAX_PASTE {
            return self.say(format!("not pasting {} bytes: at most {MAX_PASTE} go in one paste", text.len()));
        }
        let event = json!({ "session_id": t.sid, "block_id": t.block, "event": { "type": "paste", "data": text } });
        self.remote.send_then("block.event", event, self.noted());
    }

    /// A config binding: an action with a dot is ours, one without is the config's (run on the
    /// server), and a Lua function goes through `client.key` so its fallthrough rules apply.
    fn binding(&mut self, b: Bound, prefix: Vec<Chord>, last: &Chord) -> Flow {
        match b.action.as_deref() {
            Some(name) => self.action(name, b.args.as_ref()),
            None => {
                let Some(sid) = self.session_id() else { return Flow::Go };
                let block = self.target().map(|t| t.block);
                let prefix =
                    (!prefix.is_empty()).then(|| prefix.iter().map(Chord::to_string).collect::<Vec<_>>().join(">"));
                let p = json!({ "session_id": sid, "block_id": block, "key": last.to_string(), "prefix": prefix });
                self.spawn("client.key", p);
                Flow::Go
            }
        }
    }

    fn config_action(&self, name: &str, args: Option<&Value>) -> Flow {
        let t = self.target();
        let p = json!({ "action": name, "args": args, "origin": "key",
            "session_id": self.session_id(), "block_id": t.as_ref().map(|t| &t.block) });
        self.spawn("client.action", p);
        Flow::Go
    }

    /// The actions `leon attach` runs itself, named as the app's.
    fn action(&mut self, name: &str, args: Option<&Value>) -> Flow {
        // these need no pane, so they work even while the tab has none
        match (name, &self.mode) {
            (n, _) if !n.contains('.') => return self.config_action(n, args),
            ("client.detach", _) => return Flow::Done(Ok(())),
            ("session.new", _) => {
                self.attach(picker::Pick::New);
                return Flow::Go;
            }
            ("client.picker", Mode::Attached(sid)) => {
                self.mode = Mode::Picker(Picker::default(), Some(sid.clone()));
                return Flow::Go;
            }
            _ => {}
        }
        if name.starts_with("client.tab.") || name == "pane.new_floating" {
            let Some(sid) = self.session_id().map(str::to_string) else { return Flow::Go };
            let sid = &sid;
            match name {
                "client.tab.new" | "pane.new_floating" => {
                    let layout = self.target().as_ref().map(|t| self.here(t));
                    let method = if name == "client.tab.new" { "session.new_window" } else { "session.new_floating" };
                    self.act(method, json!({"session_id": sid, "layout": layout}));
                }
                "client.tab.next" | "client.tab.previous" | "client.tab.goto" => {
                    let (n, active) = self.remote.model().session(sid).map_or((0, 0), |s| (s.windows.len(), s.active));
                    let to = match (name, args.and_then(|a| a["index"].as_u64())) {
                        ("client.tab.next", _) => (active + 1) % n.max(1),
                        ("client.tab.previous", _) => (active + n.max(1) - 1) % n.max(1),
                        (_, Some(i)) if i >= 1 => i as usize - 1,
                        _ => return Flow::Go,
                    };
                    let wid = self.remote.model().session(sid).and_then(|s| s.windows.get(to).map(|w| w.id.clone()));
                    if let Some(wid) = wid {
                        self.act("session.focus_window", json!({ "session_id": sid, "window_id": wid }));
                    }
                }
                _ => self.say(format!("leon attach has no action {name}")),
            }
            return Flow::Go;
        }
        let Some(t) = self.target() else { return Flow::Go };
        let sid = &t.sid;
        match name {
            "pane.split.right" | "pane.split.down" | "pane.split.left" | "pane.split.up" | "pane.split" => {
                let way =
                    name.strip_prefix("pane.split.").or(args.and_then(|a| a["direction"].as_str())).unwrap_or("right");
                let (direction, side) = match way {
                    "left" => ("horizontal", "before"),
                    "up" => ("vertical", "before"),
                    "down" => ("vertical", "after"),
                    _ => ("horizontal", "after"),
                };
                if t.zoomed {
                    self.act("session.zoom_block", json!({ "session_id": sid, "block_id": t.block, "zoom": false }));
                }
                let layout = self.here(&t);
                let p = json!({ "session_id": sid, "anchor_id": t.block, "direction": direction, "side": side,
                    "layout": layout, "focus": true });
                self.act("session.new_split", p);
            }
            "pane.focus.left" | "pane.focus.right" | "pane.focus.up" | "pane.focus.down" => {
                if t.zoomed {
                    // from the pane on screen: a zoom doesn't move focus, so the focused one may be hidden
                    self.act("session.focus_block", json!({ "session_id": sid, "block_id": t.block }));
                    self.act("session.zoom_block", json!({ "session_id": sid, "block_id": t.block, "zoom": false }));
                }
                let toward = &name["pane.focus.".len()..];
                self.act("session.focus_direction", json!({ "session_id": sid, "direction": toward }));
            }
            "pane.float" => self.act("session.float_block", json!({ "session_id": sid, "block_id": t.block })),
            "pane.zoom" => self.act("session.zoom_block", json!({ "session_id": sid, "block_id": t.block })),
            "pane.close" => self.act("block.close", json!({ "session_id": sid, "block_id": t.block })),
            other => self.say(format!("leon attach has no action {other}")),
        }
        Flow::Go
    }

    /// A layout for a shell in the target pane's working directory (or the default, `$HOME`). Over
    /// a link the model's cwd, as the app's `here` does: no wait on the link, and the path is the
    /// host's, so it can't be checked here.
    fn here(&self, t: &Target) -> Value {
        let pwd = match self.remote.is_link() {
            true => self.remote.model().block(&t.block).and_then(|(_, b)| b.cwd.clone()).map(std::path::PathBuf::from),
            false => {
                let p = json!({ "session_id": t.sid, "block_id": t.block, "method": "dev.leon.terminal.pwd" });
                let got = self.remote.call("block.method", p).ok();
                got.and_then(|r| r["path"].as_str().map(std::path::PathBuf::from))
            }
        };
        match split_cwd(pwd, self.host.is_some()) {
            Some(d) => json!({ "block": { "options": { "cwd": d } } }),
            None => json!({ "block": {} }),
        }
    }

    /// Draw the picker or the attached tab; a session that went away goes back to the picker.
    /// False when the host was resized between laying the frame out and drawing it: nothing was
    /// drawn (the buffer is the new size), so the caller draws again.
    pub(super) fn draw(&mut self, term: &mut super::host::Term) -> std::io::Result<bool> {
        term.autoresize()?;
        let size = term.size()?;
        let area = Rect::new(0, 0, size.width, size.height);
        if area.is_empty() {
            return Ok(true); // a 0-row or 0-column host: nothing fits
        }
        let mut drawn = true;
        let colors = self.colors;
        if let Mode::Picker(p, _) = &self.mode {
            let (rows, screens) = {
                let m = self.remote.model();
                (picker::rows(&m.sessions), m.screens.clone())
            };
            self.remote.watch(p.selected_row(&rows).and_then(|r| r.block.clone()).into_iter().collect());
            let note = self.reconnecting();
            term.draw(|f| match f.area() == area {
                true => {
                    p.draw(f.buffer_mut(), area, &rows, &screens, colors);
                    if let Some(n) = &note {
                        draw::corner_note(f.buffer_mut(), Rect { height: 1, ..area }, n, colors);
                    }
                }
                false => drawn = false,
            })?;
            return Ok(drawn);
        }
        let Mode::Attached(sid) = &self.mode else { return Ok(true) };
        let (session, screens) = {
            let m = self.remote.model();
            (m.session(sid).cloned(), m.screens.clone())
        };
        let Some(s) = session else {
            self.mode = Mode::Picker(Picker::default(), None);
            return self.draw(term);
        };
        let panes_area = Rect { height: area.height.saturating_sub(1), ..area };
        let w = s.windows.get(s.active);
        let overlays = w.map(|w| s.overlay(&w.id)).unwrap_or_default();
        let (tile_area, overlays) = super::overlays::layout(&overlays, panes_area);
        let laid = match w.and_then(|w| w.layout.as_ref()) {
            Some(tree) => panes::layout(
                tree,
                w.and_then(|w| w.zoomed.as_deref()),
                (tile_area.width as f32, tile_area.height as f32),
                1.,
            ),
            None => panes::Layout::default(),
        };
        // run inside a pane on screen, the client neither sizes its own pane (its terminal would
        // shrink, and it would ask again, down to one row) nor draws it into itself
        let own = std::env::var("LEON_BLOCK").ok();
        let shown: Vec<String> = laid
            .panes
            .iter()
            .map(|p| p.block_id.clone())
            .chain(overlays.iter().map(|p| p.id.clone()))
            .filter(|id| Some(id) != own.as_ref())
            .collect();
        // the size rule: resize a pane when this client's own size for it changes (or it comes
        // back on screen), never because another client resized it
        self.asked.retain(|id, _| shown.contains(id));
        self.remote.watch(shown.clone());
        for p in laid.panes.iter().filter(|p| shown.contains(&p.block_id)) {
            let r = draw::cells(&p.rect, tile_area);
            let want = (r.height.max(1), r.width.max(2));
            if self.asked.get(&p.block_id) != Some(&want) {
                self.remote.resize(&s.id, &p.block_id, want.0, want.1);
                self.asked.insert(p.block_id.clone(), want);
            }
        }
        for p in overlays.iter().filter(|p| shown.contains(&p.id)) {
            let want = (p.inner.height.max(1), p.inner.width.max(2));
            if self.asked.get(&p.id) != Some(&want) {
                self.remote.resize(&s.id, &p.id, want.0, want.1);
                self.asked.insert(p.id.clone(), want);
            }
        }
        let focus = w.and_then(super::overlays::focused).map(str::to_string);
        let tabs = s.windows.iter().enumerate().map(|(i, w)| (picker::tab_name(&s, w), i == s.active)).collect();
        // while the link is down actions fail at once: their notes mustn't hide why
        let note = self.reconnecting().or_else(|| super::lock(&self.note).clone());
        let index = self.remote.model().sessions.iter().position(|x| x.id == s.id).unwrap_or(0);
        let name = picker::session_name(&s, index);
        let bar = Bar {
            session: &name,
            tabs,
            pending: self.after_prefix,
            prefix: &self.keys.prefix_name,
            note: note.as_deref(),
        };
        term.draw(|f| {
            if f.area() != area {
                drawn = false;
                return;
            }
            let buf = f.buffer_mut();
            let mut cursor = None;
            for p in &laid.panes {
                let r = draw::cells(&p.rect, tile_area);
                if own.as_ref() == Some(&p.block_id) {
                    let style = ratatui::style::Style::default().fg(colors.rgb(draw::brand::UMBER));
                    buf.set_stringn(r.x, r.y, "(this terminal: leon attach)", r.width as usize, style);
                    continue;
                }
                let Some(frame) = screens.get(&p.block_id) else { continue };
                draw::screen(buf, r, frame, colors);
                if focus.as_deref() == Some(p.block_id.as_str()) {
                    cursor = frame
                        .cursor
                        .as_ref()
                        .filter(|c| c.col < r.width as usize && c.line < r.height as usize)
                        .map(|c| (r.x + c.col as u16, r.y + c.line as u16));
                }
            }
            draw::borders(buf, tile_area, &laid.dividers, &laid.panes, focus.as_deref(), colors);
            for p in &overlays {
                if cursor.is_some_and(|(x, y)| p.outer.contains(Position { x, y })) {
                    cursor = None;
                }
                let focused = focus.as_deref() == Some(&p.id);
                super::overlays::paint(buf, p, focused, colors);
                if own.as_ref() == Some(&p.id) {
                    continue;
                }
                if let Some(frame) = screens.get(&p.id) {
                    draw::screen(buf, p.inner, frame, colors);
                    if focused {
                        cursor = frame
                            .cursor
                            .as_ref()
                            .filter(|c| c.col < p.inner.width as usize && c.line < p.inner.height as usize)
                            .map(|c| (p.inner.x + c.col as u16, p.inner.y + c.line as u16));
                    }
                }
            }
            draw::bar(buf, Rect { y: area.height.saturating_sub(1), height: 1, ..area }, &bar, colors);
            if let Some((x, y)) = cursor {
                f.set_cursor_position(Position { x, y });
            }
        })?;
        Ok(drawn)
    }
}

/// The directory a split starts in: the pane's, when it is still there. Over `-H` it is a path on
/// the host, which this disk can't vouch for: the host's server checks it.
fn split_cwd(pwd: Option<std::path::PathBuf>, remote: bool) -> Option<String> {
    pwd.filter(|d| remote || d.is_dir()).and_then(|d| d.to_str().map(String::from))
}

#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;
