//! Going places: the session switcher (⌘K) in the palette's box, the tabs used most recently, and
//! the jump the switcher, Activity and a clicked notification share.

use super::*;
use crate::palette::Item;
use crate::remote::Model;
use crate::switcher::Row;

/// Tabs remembered as recently used.
const RECENT: usize = 64;

/// Run `f` over every connected machine's model, with its index and label.
pub(super) fn with_models<R>(hosts: &Hosts, f: impl FnOnce(&[(usize, &str, &Model)]) -> R) -> R {
    let machines = hosts.machines();
    let guards: Vec<(usize, &str, _)> =
        hosts.remotes().map(|(i, r)| (i, machines[i].label.as_str(), r.model())).collect();
    let models: Vec<(usize, &str, &Model)> = guards.iter().map(|(i, l, m)| (*i, *l, &**m)).collect();
    f(&models)
}

impl Workspace {
    /// Note the tab on screen as the most recently used.
    pub(super) fn note_recent(&mut self) {
        let Some(w) = self.active_window() else { return };
        if self.recent.first() != Some(&w.id) {
            self.recent.retain(|id| *id != w.id);
            self.recent.insert(0, w.id);
            self.recent.truncate(RECENT);
        }
    }

    pub fn switcher_open(&self) -> bool {
        self.palette.as_ref().is_some_and(|p| p.switch)
    }

    /// Why the switcher's last ⌘↩ couldn't split, while it says so.
    pub fn switcher_note(&self) -> Option<String> {
        self.palette.as_ref().filter(|p| p.switch || p.themes.is_some())?.note.clone()
    }

    /// The open switcher's rows as shown (`host › session › tab`, ` › block` for a block row).
    pub fn switcher_rows(&self) -> Vec<String> {
        self.palette_matches().into_iter().map(|it| it.title).collect()
    }

    /// The switcher's rows for `query` as palette items: each runs `session.switch` with where it goes.
    pub(super) fn switcher_items(&self, query: &str) -> Vec<Item> {
        let rows = with_models(&self.hosts, |models| {
            let (tabs, blocks) = switcher::tabs(models);
            switcher::rank(&tabs, &blocks, query, &self.recent)
        });
        let frame = self.pulse();
        rows.into_iter()
            .map(|r: Row| Item {
                name: "session.switch".into(),
                title: r.text(),
                keywords: Vec::new(),
                shortcut: r.status.map(|s| status::glyph(s, frame)),
                args: Some(json!({
                    "host": r.host, "session_id": r.session_id, "window_id": r.window_id, "block_id": r.block_id,
                })),
            })
            .collect()
    }

    /// `session.switch`: with no place, open (or close) the switcher; with one, go there, or with
    /// `split` move its pane beside the focused one when both are on this machine (a block shows
    /// in one place only; from another session it joins this one). On another machine's row
    /// `split` keeps the switcher open and says why it can't.
    pub(super) fn switch_action(&mut self, args: Option<&Value>, cx: &mut Context<Self>) {
        let Some(a) = args.filter(|a| a["session_id"].is_string()) else {
            self.palette = match self.palette {
                Some(_) => None,
                None => Some(Palette {
                    query: String::new(),
                    selected: 0,
                    ask: None,
                    switch: true,
                    note: None,
                    themes: None,
                }),
            };
            cx.notify();
            return;
        };
        let host = a["host"].as_u64().unwrap_or(0) as usize;
        let (sid, wid, block) = (a["session_id"].as_str(), a["window_id"].as_str(), a["block_id"].as_str());
        let sid = sid.unwrap_or_default().to_string();
        if a["split"] == true && host != self.host {
            let machines = self.hosts.machines();
            let label = |i: usize| machines.get(i).map_or("another machine", |m| m.label.as_str());
            let (there, here) = (label(host), label(self.host));
            let note = format!("Can't split: that pane is on {there}, and a pane joins only a window on its own machine ({here} here). ↩ goes there.");
            match self.palette.as_mut() {
                Some(p) => p.note = Some(note), // ⌘↩ left it open: the row stays picked
                None => {
                    self.palette = Some(Palette {
                        query: String::new(),
                        selected: 0,
                        ask: None,
                        switch: true,
                        note: Some(note),
                        themes: None,
                    })
                }
            }
            cx.notify();
            return;
        }
        self.palette = None;
        if a["split"] == true && host == self.host {
            let moved = block.map(String::from).or_else(|| {
                let m = self.app.model();
                let w = m.session(&sid)?.windows.iter().find(|w| Some(w.id.as_str()) == wid)?.clone();
                target(&w).map(String::from)
            });
            // a floating or docked focus can't be an anchor: join beside a tiled pane instead
            let here = self.target().and_then(|(sid, b, _)| {
                let m = self.app.model();
                let s = m.session(&sid)?;
                let l = s.windows.get(self.active_of(s))?.layout.as_ref()?;
                Some(if l.contains(&b) { b } else { l.leaves().first()?.to_string() })
            });
            if let (Some(moved), Some(here)) = (moved, here) {
                if moved != here {
                    let p = json!({ "block_id": moved, "anchor_block_id": here });
                    self.call("session.move_block", p, cx);
                    return;
                }
            }
        }
        self.go_to(host, &sid, wid, block, cx);
    }

    /// Show session `sid` on machine `host`: its tab `window`, and `block` focused when given.
    pub(super) fn go_to(
        &mut self,
        host: usize,
        sid_given: &str,
        window: Option<&str>,
        block: Option<&str>,
        cx: &mut Context<Self>,
    ) {
        self.use_host(host);
        if self.host != host {
            return; // that machine isn't connected now
        }
        // a block can have joined another session since `sid` was read (a notice, a stale row)
        let sid = block.and_then(|b| self.app.model().block(b).map(|(s, _)| s.id.clone()));
        self.session_id = Some(sid.unwrap_or_else(|| sid_given.to_string()));
        match (block, window) {
            (None, None) => cx.notify(),
            (b, w) => self.ask_focus(w.map(String::from), b, cx),
        }
    }
}
