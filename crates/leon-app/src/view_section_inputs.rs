//! What each section draws (view_sections.rs), built from the workspace: the sidebar's, the title
//! bar's and each pane header's input, exactly what its helper reads.

use std::collections::hash_map::DefaultHasher;

use super::header::{HEADER_PARTS, TITLE_MAX};
use super::*;
use crate::attention;
use crate::glass::Glass;
use crate::hosts::HostState;

/// The actions whose shortcuts the sections' tooltips show.
const TIPPED: [&str; 11] = [
    "client.overview.toggle",
    "client.palette.toggle",
    "client.sidebar.activity.toggle",
    "client.sidebar.toggle",
    "client.tab.new",
    "session.new",
    "pane.split.right",
    "pane.split.down",
    "pane.zoom",
    "pane.unfloat",
    "pane.close",
];

/// What the sidebar draws (`render_sidebar`, its toolbar, host labels and Activity list).
#[derive(Clone, PartialEq, Debug)]
pub(super) struct SidebarInput {
    pub(super) theme: Theme,
    /// How see-through its fill is (`chrome_fill`).
    pub(super) glass: Glass,
    pub(super) list: SidebarList,
    pub(super) rows: Vec<SessionRow>,
    pub(super) selected: Option<String>,
    pub(super) collapsed: Vec<String>,
    pub(super) grouped: bool,
    pub(super) hosts: Vec<HostLabel>,
    pub(super) width: f32,
    pub(super) tips: u64,
    pub(super) badge: Option<String>,
    /// The Activity list's rows with their waited times, while it shows.
    pub(super) activity: Vec<(attention::Row, String)>,
    pub(super) working: bool,
}

/// A machine's label in the sidebar.
#[derive(Clone, PartialEq, Debug)]
pub(super) struct HostLabel {
    pub(super) label: String,
    pub(super) state: HostState,
    /// Collapsed, the label counts what it hides.
    pub(super) sessions: Option<usize>,
    pub(super) collapsed: bool,
    pub(super) tailnet: Option<String>,
}

/// What the title bar draws (`render_titlebar`).
#[derive(Clone, PartialEq, Debug)]
pub(super) struct TitleBarInput {
    pub(super) theme: Theme,
    pub(super) glass: Glass,
    pub(super) row: Option<SessionRow>,
    pub(super) top_tabs: bool,
    /// The sidebar is hidden: the toolbar's buttons are here.
    pub(super) hidden: bool,
    pub(super) tips: u64,
    pub(super) badge: Option<String>,
    pub(super) working: bool,
}

/// What a pane's header draws (`render_header`, its find bar).
#[derive(Clone, PartialEq, Debug)]
pub(super) struct HeaderInput {
    pub(super) id: String,
    /// The pane's theme (faded with it) and the window's (its tooltips).
    pub(super) theme: Theme,
    pub(super) window_theme: Theme,
    pub(super) state: Option<Indicator>,
    /// The program in the pane, whose icon leads the header.
    pub(super) program: Kind,
    pub(super) caption: Option<String>,
    pub(super) host: Option<String>,
    /// The title as cut to the pane's width, and the whole of it when cut.
    pub(super) title: (String, Option<String>),
    pub(super) focused: bool,
    pub(super) hovered: bool,
    pub(super) overlay: bool,
    /// The find bar, while open here: the query, case, regex and the count.
    pub(super) find: Option<(String, bool, bool, String)>,
    pub(super) family: SharedString,
    pub(super) tips: u64,
}

impl Workspace {
    /// A hash of the tooltips' shortcuts.
    pub(super) fn tips_key(&self) -> u64 {
        let mut h = DefaultHasher::new();
        TIPPED.iter().for_each(|a| self.keymap.shortcut(a).hash(&mut h));
        h.finish()
    }

    pub(super) fn sidebar_input(&self, rows: &[SessionRow]) -> Option<SidebarInput> {
        let list = self.places().sidebar.filter(|_| !self.stopped && !self.overview_shown())?;
        let working = |i: &Option<Indicator>| matches!(i, Some(Indicator::Working { .. }));
        let now = Instant::now();
        let activity = match list {
            SidebarList::Activity => self
                .attention
                .rows()
                .into_iter()
                .map(|r| (attention::waited(r.since, now), r))
                .map(|(w, r)| (r, w))
                .collect(),
            _ => Vec::new(),
        };
        let mut collapsed: Vec<String> = self.collapsed.iter().cloned().collect();
        collapsed.sort();
        let hosts: Vec<HostLabel> = self
            .hosts
            .machines()
            .iter()
            .map(|m| HostLabel {
                label: m.label.clone(),
                state: m.state(),
                sessions: m.remote().map(|r| r.model().sessions.len()).filter(|n| *n > 0),
                collapsed: self.host_collapsed(&m.label),
                tailnet: self.tailnet_line(m),
            })
            .collect();
        let (grouped, selected) = (self.grouped(), self.session_id.clone());
        // the statuses `render_sidebar` draws: none under a collapsed host, the selected session's
        // none in top style, a closed session's own, an open one's tabs'
        let drawn = |r: &SessionRow| match list {
            SidebarList::Activity => false,
            _ if grouped && hosts.get(r.host).is_some_and(|h| h.collapsed) => false,
            SidebarList::SessionsOnly => selected.as_ref() != Some(&r.id) && working(&r.status),
            _ if collapsed.contains(&r.id) => working(&r.status),
            _ => r.windows.iter().any(|w| working(&w.status)),
        };
        Some(SidebarInput {
            theme: self.theme,
            glass: self.glass(),
            list,
            working: rows.iter().any(drawn)
                || activity.iter().any(|(r, _)| matches!(r.status, Indicator::Working { .. })),
            rows: rows.to_vec(),
            selected,
            collapsed,
            grouped,
            hosts,
            width: self.settings.sidebar_width,
            tips: self.tips_key(),
            badge: self.activity_badge(),
            activity,
        })
    }

    pub(super) fn titlebar_input(&self, rows: &[SessionRow]) -> Option<TitleBarInput> {
        if !self.titlebar_shown() || self.stopped || self.overview_shown() {
            return None;
        }
        let places = self.places();
        let row = rows.iter().find(|r| Some(&r.id) == self.session_id.as_ref()).cloned();
        let working = places.top_tabs
            && row
                .as_ref()
                .is_some_and(|r| r.windows.iter().any(|w| matches!(w.status, Some(Indicator::Working { .. }))));
        Some(TitleBarInput {
            theme: self.theme,
            glass: self.glass(),
            row,
            top_tabs: places.top_tabs,
            hidden: places.sidebar.is_none(),
            tips: self.tips_key(),
            badge: self.activity_badge(),
            working,
        })
    }

    /// The headers drawn now, with each pane's width (as `render_panes` lays them out).
    pub(super) fn header_inputs(&self, window: &Window, cx: &App) -> Vec<HeaderInput> {
        if self.stopped || self.overview_shown() {
            return Vec::new();
        }
        let Some((w, terms)) = self.shown() else { return Vec::new() };
        self.pane_rects(&w, window)
            .into_iter()
            .filter(|(id, _, kind)| terms.contains(id) && self.header_shown(id, kind))
            .map(|(id, rect, _)| self.header_input(&id, self.header_width(&id, rect.w), cx))
            .collect()
    }

    /// What pane `id`'s header draws in a pane `width` wide.
    pub(super) fn header_input(&self, id: &str, width: f32, cx: &App) -> HeaderInput {
        let title = self.header_title(id, (width - HEADER_PARTS - self.badge_width(id)).min(TITLE_MAX), cx);
        let find = self.term.find.as_ref().filter(|f| f.block == id);
        // its own statement: the model's guard would live to the end of the struct, and
        // `header_caption` below locks the model again
        let state = self.app.model().block(id).and_then(|(_, b)| status::indicator_of(&b.newest_first()));
        let program = self.app.model().block(id).map_or(Kind::Shell, |(_, b)| crate::program_kind::kind_of_block(b));
        HeaderInput {
            theme: self.pane_theme(id),
            window_theme: self.theme,
            state,
            program,
            caption: self.header_caption(id),
            host: self.pane_host(id),
            title,
            focused: self.target().is_some_and(|(_, t, _)| t == id),
            hovered: self.hovered_pane.as_deref() == Some(id),
            overlay: self.is_overlay(id),
            find: find.map(|f| (f.query.clone(), f.case, f.regex, self.find_count().unwrap_or_default())),
            family: self.font_family.clone(),
            tips: self.tips_key(),
            id: id.to_string(),
        }
    }

    /// Whether pane `id` (a tile, a dock or a float) draws a header: always on a float or drawer,
    /// on a tile with headers on, or while its find bar is open.
    pub(super) fn header_shown(&self, id: &str, kind: &str) -> bool {
        self.settings.pane_headers || kind != "pane" || self.term.find.as_ref().is_some_and(|f| f.block == id)
    }
}
