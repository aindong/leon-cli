//! What the window draws, read from the model copy: rows for sessions and tabs, titles, status,
//! the settings, and the signature the poll compares to decide on a redraw.

use super::*;

impl Workspace {
    /// Every machine's sessions, This Mac's first, then each host's in the hosts list's order.
    pub(super) fn rows(&self) -> Vec<SessionRow> {
        let mut out = Vec::new();
        for (host, remote) in self.hosts.remotes() {
            out.extend(Self::host_rows_of(host, &remote.model()));
        }
        out
    }

    fn host_rows_of(host: usize, m: &crate::remote::Model) -> Vec<SessionRow> {
        m.sessions
            .iter()
            .enumerate()
            .map(|(i, s)| {
                // each block read once: per window per block would be a million a poll at the caps
                let mut by_window = std::collections::HashMap::<&str, Option<Indicator>>::new();
                for b in &s.blocks {
                    let shown = status::indicator_of(&b.newest_first());
                    let w = by_window.entry(&b.window_id).or_default();
                    *w = status::aggregate([*w, shown]);
                }
                let windows: Vec<WindowRow> = s
                    .windows
                    .iter()
                    .map(|w| WindowRow {
                        id: w.id.clone(),
                        title: switcher::tab_title(s, w),
                        status: by_window.get(w.id.as_str()).copied().flatten(),
                    })
                    .collect();
                SessionRow {
                    host,
                    id: s.id.clone(),
                    title: switcher::session_title(s, i),
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
        Some(match self.pane_place(block_id)? {
            Err(title) => title,
            Ok((program, None)) => program,
            Ok((program, Some(cwd))) => format!("{program} {cwd}"),
        })
    }

    /// The header's title in `room` px, and the whole of it for a tooltip when it was cut: a
    /// directory that doesn't fit loses its middle (`chrome::elide_path`). A title the program
    /// set is its own; GPUI's ellipsis ends it if it must.
    pub fn header_title(&self, block_id: &str, room: f32, cx: &App) -> (String, Option<String>) {
        let full = self.pane_title(block_id).unwrap_or_default();
        let Some(Ok((program, Some(cwd)))) = self.pane_place(block_id) else { return (full, None) };
        let mut lines = cx.text_system().line_wrapper(gpui::font(".SystemUIFont"), px(SECONDARY_TEXT));
        let mut fits = |cwd: &str| {
            lines.wrap_line(&[gpui::LineFragment::text(&format!("{program} {cwd}"))], px(room)).next().is_none()
        };
        let shown = format!("{program} {}", chrome::elide_path(&cwd, &mut fits));
        let tip = (shown != full).then_some(full);
        (shown, tip)
    }

    /// The title the block's program set (Err), else its program and where it is, from `~`.
    fn pane_place(&self, block_id: &str) -> Option<Result<(String, Option<String>), String>> {
        let m = self.app.model();
        let b = m.block(block_id)?.1;
        if let Some(t) = &b.title {
            return Some(Err(t.clone()));
        }
        let home = leon_core::paths::home().map(|h| h.display().to_string());
        let cwd = b.cwd.as_deref().map(|d| match home.as_deref().and_then(|h| d.strip_prefix(h)) {
            Some(rest) if rest.is_empty() || rest.starts_with('/') => format!("~{rest}"),
            _ => d.to_string(),
        });
        Some(Ok((program_name(&b.command), cwd)))
    }

    pub fn settings(&self) -> Settings {
        self.settings.clone()
    }

    /// Where tabs and sessions are drawn now.
    pub fn places(&self) -> Places {
        // with no session and no host the sidebar has nothing to list: the empty state takes the window
        let listed = self.grouped() || self.hosts.remotes().any(|(_, r)| !r.model().sessions.is_empty());
        let mut places = chrome::places(self.settings.tab_style, self.sidebar && listed || self.activity);
        if self.activity {
            places.sidebar = Some(SidebarList::Activity);
        }
        places
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
        self.empty_items().into_iter().map(|(keys, what, _)| (keys, what)).collect()
    }

    /// The empty state's rows: (keys, what they do, the action a click runs).
    pub(super) fn empty_items(&self) -> Vec<(String, String, Option<&'static str>)> {
        let key = |a: &str| self.keymap.shortcut(a).unwrap_or_else(|| a.to_string());
        vec![
            (key("session.new"), "New session".into(), Some("session.new")),
            (
                key("client.palette.toggle"),
                "Command palette: every action, yours too".into(),
                Some("client.palette.toggle"),
            ),
            ("leon skill setup".into(), "Let Claude Code and Codex open panes here".into(), None),
            ("leon hosts add".into(), "Add a host".into(), Some("client.host.add")),
        ]
    }

    /// The working pulse's frame, from the clock.
    pub(super) fn pulse(&self) -> u64 {
        (self.epoch.elapsed().as_millis() / SPIN.as_millis()) as u64
    }

    /// The pulse's frame while a working indicator is drawn: in the sidebar's rows, the top tabs,
    /// or a visible pane's header (its own state, which a blocked pane beside it would hide).
    pub(super) fn spinning(&self, rows: &[SessionRow]) -> Option<u64> {
        let working = |i: &Option<Indicator>| matches!(i, Some(Indicator::Working { .. }));
        let places = self.places();
        let sidebar = places.sidebar.is_some()
            && rows.iter().any(|r| working(&r.status) || r.windows.iter().any(|w| working(&w.status)));
        let selected = rows.iter().filter(|r| Some(&r.id) == self.session_id.as_ref());
        let tabs = places.top_tabs && selected.flat_map(|r| &r.windows).any(|w| working(&w.status));
        let headers = {
            let shown = self.shown().map(|(_, ids)| ids).unwrap_or_default();
            let m = self.app.model();
            // a float's or drawer's header shows even with headers off
            let header = |s: &crate::remote::Session, b: &crate::remote::Block| {
                self.settings.pane_headers || s.overlays.get(&b.window_id).is_some_and(|o| o.contains(&b.id))
            };
            shown.iter().any(|id| {
                m.block(id).is_some_and(|(s, b)| header(s, b) && working(&status::indicator_of(&b.newest_first())))
            })
        };
        (sidebar || tabs || headers).then(|| self.pulse())
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
        for (host, remote) in self.hosts.remotes() {
            let m = remote.model();
            host.hash(&mut h);
            for s in &m.sessions {
                (&s.id, &s.label, s.active, s.revision).hash(&mut h);
                for w in &s.windows {
                    (&w.id, &w.label).hash(&mut h);
                }
            }
        }
        {
            let m = self.app.model();
            for id in &shown {
                (id, m.screen_updates.get(id)).hash(&mut h);
            }
        }
        for m in self.hosts.machines() {
            (&m.label, m.state()).hash(&mut h);
        }
        (self.host, &self.settings.collapsed_hosts).hash(&mut h);
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
        self.find_due().hash(&mut h);
        self.activity_signature(&mut h);
        // each header draws its own pane's state, which its tab's may hide
        for id in &shown {
            let state = self.app.model().block(id).and_then(|(_, b)| status::indicator_of(&b.newest_first()));
            (state, self.header_caption(id), self.pane_title(id)).hash(&mut h);
        }
        h.finish()
    }
}
