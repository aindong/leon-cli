//! What the window draws, read from the model copy: rows for sessions and tabs, titles, status,
//! the settings, and the signature the poll compares to decide on a redraw.

use super::*;

impl Workspace {
    pub(super) fn rows(&self) -> Vec<SessionRow> {
        let m = self.app.model();
        m.sessions
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let shows = |b: &crate::remote::Block| status::indicator_of(&b.newest_first());
                let windows: Vec<WindowRow> = s
                    .windows
                    .iter()
                    .map(|w| WindowRow {
                        id: w.id.clone(),
                        title: w.label.clone().unwrap_or_else(|| {
                            let shown = target(w).and_then(|id| s.blocks.iter().find(|b| b.id == id));
                            shown
                                .map_or("empty".into(), |b| b.title.clone().unwrap_or_else(|| program_name(&b.command)))
                        }),
                        status: status::aggregate(s.blocks.iter().filter(|b| b.window_id == w.id).map(shows)),
                    })
                    .collect();
                SessionRow {
                    id: s.id.clone(),
                    title: s.label.clone().unwrap_or_else(|| format!("Session {}", i + 1)),
                    status: status::aggregate(windows.iter().map(|w| w.status)),
                    windows,
                    active: s.active,
                }
            })
            .collect()
    }

    /// (each session's indicator, each tab's of the selected session), as the sidebar and tabs show.
    pub fn indicators(&self) -> (Vec<Option<Indicator>>, Vec<Option<Indicator>>) {
        let rows = self.rows();
        let tabs = rows.iter().find(|r| Some(&r.id) == self.session_id.as_ref());
        let tabs = tabs.map(|r| r.windows.iter().map(|w| w.status).collect()).unwrap_or_default();
        (rows.iter().map(|r| r.status).collect(), tabs)
    }

    /// The selected session's tab titles: a tab's label, else the title its program set, else the program.
    pub fn tab_titles(&self) -> Vec<String> {
        let rows = self.rows();
        let row = rows.into_iter().find(|r| Some(&r.id) == self.session_id.as_ref());
        row.map(|r| r.windows.into_iter().map(|w| w.title).collect()).unwrap_or_default()
    }

    /// The status caption a pane's header shows.
    pub fn pane_caption(&self, block_id: &str) -> Option<String> {
        status::caption_of(&self.app.model().block(block_id)?.1.newest_first())
    }

    /// What a pane's header says after its status icon: the program's caption, else the state's word.
    pub fn header_caption(&self, block_id: &str) -> Option<String> {
        let m = self.app.model();
        let r = m.block(block_id)?.1.newest_first();
        status::caption_of(&r).or_else(|| status::indicator_of(&r).map(|i| status::word(i).to_string()))
    }

    /// What a pane's header names it: the title its program set, else the program and where it is.
    pub fn pane_title(&self, block_id: &str) -> Option<String> {
        let m = self.app.model();
        let b = m.block(block_id)?.1;
        if let Some(t) = &b.title {
            return Some(t.clone());
        }
        let home = leon_core::paths::home().map(|h| h.display().to_string());
        let cwd = b.cwd.as_deref().map(|d| match home.as_deref().and_then(|h| d.strip_prefix(h)) {
            Some(rest) if rest.is_empty() || rest.starts_with('/') => format!("~{rest}"),
            _ => d.to_string(),
        });
        Some([Some(program_name(&b.command)), cwd].into_iter().flatten().collect::<Vec<_>>().join(" "))
    }

    pub fn settings(&self) -> Settings {
        self.settings
    }

    /// Where tabs and sessions are drawn now.
    pub fn places(&self) -> Places {
        // with no session the sidebar has nothing to list: the empty state takes the window
        let sessions = !self.app.model().sessions.is_empty();
        chrome::places(self.settings.tab_style, self.sidebar && sessions)
    }

    /// Save the settings, when this workspace has a place for them.
    pub(super) fn save_settings(&self) {
        if let Some(p) = &self.settings_path {
            if let Err(e) = self.settings.save(p) {
                eprintln!("leon: can't save the settings to {}: {e}", p.display());
            }
        }
    }

    /// The empty state's hints, (keys, what they do), with keys from the live keymap.
    pub fn empty_hints(&self) -> Vec<(String, String)> {
        let key = |a: &str| self.keymap.shortcut(a).unwrap_or_else(|| a.to_string());
        vec![
            (key("session.new"), "New session".into()),
            (key("client.palette.toggle"), "Command palette: every action, yours too".into()),
            ("leon skill setup".into(), "Let Claude Code and Codex open panes here".into()),
        ]
    }

    /// The spinner frame, when anything is working (a session's status covers its tabs and panes).
    pub(super) fn spinning(&self, rows: &[SessionRow]) -> Option<u64> {
        let working = rows.iter().any(|r| matches!(r.status, Some(Indicator::Working { .. })));
        working.then(|| (self.epoch.elapsed().as_millis() / SPIN.as_millis()) as u64)
    }

    pub fn font_size(&self) -> f32 {
        self.font_size
    }

    /// Changes worth a redraw: every session's view (its revision moves with layout, focus, zoom
    /// and labels) and the screen of every pane on screen (only those are watched, so hidden tabs
    /// and panes hidden by a zoom don't count).
    pub fn signature(&self) -> u64 {
        let mut h = DefaultHasher::new();
        let shown = self.shown().map(|(_, ids)| ids).unwrap_or_default();
        {
            let m = self.app.model();
            for s in &m.sessions {
                (&s.id, &s.label, s.active, s.revision).hash(&mut h);
                for w in &s.windows {
                    (&w.id, &w.label).hash(&mut h);
                }
            }
            for id in &shown {
                (id, m.screen_updates.get(id)).hash(&mut h);
            }
        }
        // status and titles show for every tab and session, shown or not
        let rows = self.rows();
        for r in &rows {
            r.status.hash(&mut h);
            r.windows.iter().for_each(|w| (w.status, &w.title).hash(&mut h));
        }
        self.spinning(&rows).hash(&mut h);
        (self.sidebar, self.font_size.to_bits(), self.pending.len(), self.palette.is_some()).hash(&mut h);
        let st = &self.settings;
        (st.tab_style, st.pane_headers, st.sidebar_width.to_bits()).hash(&mut h);
        self.session_id.hash(&mut h);
        for id in &shown {
            (self.pane_caption(id), self.pane_title(id)).hash(&mut h);
        }
        h.finish()
    }
}
