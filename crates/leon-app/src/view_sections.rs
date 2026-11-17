//! The sidebar, the title bar and each pane header as cached views ("sections", ADR R3). A stop-gap
//! the ADR names under R4: each section holds a typed input of exactly what its helper reads and
//! renders through the workspace (`ws.update`), so the helpers, their listeners and their colours
//! stay where they are; the workspace notifies a section only when its input changed. Phase 5's
//! model entities, observed by the sections, replace it.

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;

use gpui::{AnyView, StyleRefinement, WeakEntity};

use super::*;
use crate::attention;
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

#[derive(Clone, PartialEq, Debug)]
pub(super) enum Input {
    Sidebar(SidebarInput),
    TitleBar(TitleBarInput),
    Header(HeaderInput),
}

impl Input {
    fn working(&self) -> bool {
        match self {
            Input::Sidebar(i) => i.working,
            Input::TitleBar(i) => i.working,
            Input::Header(i) => matches!(i.state, Some(Indicator::Working { .. })),
        }
    }
}

/// What outside the UI thread an input was read under: every machine's model fetches (a reader
/// thread rewrites the sessions), the hosts' states (their link threads), the tailnet's reads (its
/// thread) and the Activity list's waited times (the clock, each on its own phase).
type Stamp = u64;

/// A section: what it draws, rendered through the workspace.
pub(super) struct Section {
    ws: WeakEntity<Workspace>,
    name: String,
    input: Input,
    stamp: Stamp,
    renders: Rc<RefCell<HashMap<String, u64>>>,
    /// Observing the Pulse, while a working indicator is drawn here.
    pulse: Option<Subscription>,
}

impl Section {
    fn follow_pulse(&mut self, pulse: &gpui::Entity<super::pulse_view::Pulse>, cx: &mut Context<Self>) {
        match (self.input.working(), self.pulse.is_some()) {
            (true, false) => self.pulse = Some(cx.observe(pulse, |_, _, cx| cx.notify())),
            (false, true) => self.pulse = None,
            _ => {}
        }
    }
}

impl Render for Section {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        drawing(cx);
        *self.renders.borrow_mut().entry(self.name.clone()).or_default() += 1;
        let (input, stamp, name) = (self.input.clone(), self.stamp, self.name.clone());
        let drawn = self.ws.update(cx, |ws, cx| {
            // compared only if nothing off the UI thread moved since the sync, before the fresh
            // read or during it (a waited time turning, a slide ending on the clock)
            if cfg!(debug_assertions) && ws.input_stamp() == stamp {
                let fresh = ws.input_of(&name, window, cx);
                if ws.input_stamp() == stamp {
                    debug_assert_eq!(fresh.as_ref(), Some(&input), "section {name} drew from a stale input");
                }
            }
            ws.draw_section(&input, cx)
        });
        drawn.unwrap_or_else(|_| div().into_any_element())
    }
}

/// A shown section's view and what it was last given.
pub(super) struct SectionSlot {
    view: gpui::Entity<Section>,
    input: Input,
}

impl Workspace {
    pub(super) fn input_stamp(&self) -> Stamp {
        let mut h = DefaultHasher::new();
        self.hosts.remotes().for_each(|(i, r)| (i, r.model().fetches).hash(&mut h));
        self.hosts.machines().iter().for_each(|m| m.state().hash(&mut h));
        self.hosts.tailnet_reads().hash(&mut h);
        self.shown().map(|(_, ids)| ids).hash(&mut h); // a drawer's slide ends on the clock
        if self.activity {
            let now = Instant::now();
            self.attention.rows().iter().for_each(|r| attention::waited(r.since, now).hash(&mut h));
        }
        h.finish()
    }

    /// A hash of the tooltips' shortcuts.
    fn tips_key(&self) -> u64 {
        let mut h = DefaultHasher::new();
        TIPPED.iter().for_each(|a| self.keymap.shortcut(a).hash(&mut h));
        h.finish()
    }

    fn sidebar_input(&self, rows: &[SessionRow]) -> Option<SidebarInput> {
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
        Some(SidebarInput {
            theme: self.theme,
            list,
            rows: rows.to_vec(),
            selected: self.session_id.clone(),
            collapsed,
            grouped: self.grouped(),
            hosts: self
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
                .collect(),
            width: self.settings.sidebar_width,
            tips: self.tips_key(),
            badge: self.activity_badge(),
            working: rows.iter().any(|r| working(&r.status) || r.windows.iter().any(|w| working(&w.status)))
                || activity.iter().any(|(r, _)| matches!(r.status, Indicator::Working { .. })),
            activity,
        })
    }

    fn titlebar_input(&self, rows: &[SessionRow]) -> Option<TitleBarInput> {
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
            row,
            top_tabs: places.top_tabs,
            hidden: places.sidebar.is_none(),
            tips: self.tips_key(),
            badge: self.activity_badge(),
            working,
        })
    }

    /// The headers drawn now, with each pane's width (as `render_panes` lays them out).
    fn header_inputs(&self, window: &Window, cx: &App) -> Vec<HeaderInput> {
        if self.stopped || self.overview_shown() {
            return Vec::new();
        }
        let Some((w, terms)) = self.shown() else { return Vec::new() };
        self.pane_rects(&w, window)
            .into_iter()
            .filter(|(id, _, kind)| terms.contains(id) && self.header_shown(id, kind))
            .map(|(id, rect, _)| self.header_input(&id, rect.w, cx))
            .collect()
    }

    /// What pane `id`'s header draws in a pane `width` wide.
    pub(super) fn header_input(&self, id: &str, width: f32, cx: &App) -> HeaderInput {
        let title = self.header_title(id, (width - HEADER_PARTS - self.badge_width(id)).min(TITLE_MAX), cx);
        let find = self.term.find.as_ref().filter(|f| f.block == id);
        // its own statement: the model's guard would live to the end of the struct, and
        // `header_caption` below locks the model again
        let state = self.app.model().block(id).and_then(|(_, b)| status::indicator_of(&b.newest_first()));
        HeaderInput {
            theme: self.pane_theme(id),
            window_theme: self.theme,
            state,
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

    /// Every section's input now, by name.
    fn section_inputs(&self, window: &Window, cx: &App) -> Vec<(String, Input)> {
        let rows = self.rows();
        let mut out = Vec::new();
        out.extend(self.sidebar_input(&rows).map(|i| ("sidebar".to_string(), Input::Sidebar(i))));
        out.extend(self.titlebar_input(&rows).map(|i| ("titlebar".to_string(), Input::TitleBar(i))));
        out.extend(self.header_inputs(window, cx).into_iter().map(|i| (format!("header-{}", i.id), Input::Header(i))));
        out
    }

    /// Section `name`'s input now (the debug check), building only that one.
    fn input_of(&self, name: &str, window: &Window, cx: &App) -> Option<Input> {
        match name {
            "sidebar" => self.sidebar_input(&self.rows()).map(Input::Sidebar),
            "titlebar" => self.titlebar_input(&self.rows()).map(Input::TitleBar),
            _ => {
                let id = name.strip_prefix("header-")?;
                if self.stopped || self.overview_shown() {
                    return None;
                }
                let (w, terms) = self.shown()?;
                let (_, rect, kind) = self.pane_rects(&w, window).into_iter().find(|(p, _, _)| p == id)?;
                (terms.iter().any(|t| t == id) && self.header_shown(id, kind))
                    .then(|| Input::Header(self.header_input(id, rect.w, cx)))
            }
        }
    }

    /// Give each section what it draws now, notifying the ones that changed; make the sections that
    /// came on screen and drop those that left; run the Pulse while a shown indicator works. After
    /// `prepare`, with the grids; it notifies only sections, never the workspace.
    pub(super) fn sync_sections(&mut self, window: &Window, cx: &mut Context<Self>) {
        // a pane gone from under the mouse sent no leave
        let shown = self.shown().map(|(_, ids)| ids).unwrap_or_default();
        self.hovered_pane = self.hovered_pane.take().filter(|id| shown.contains(id));
        let stamp = self.input_stamp();
        let wanted = self.section_inputs(window, cx);
        self.sections.retain(|n, _| wanted.iter().any(|(w, _)| w == n));
        self.section_renders.borrow_mut().retain(|n, _| wanted.iter().any(|(w, _)| w == n));
        let pulse = self.pulse.clone();
        for (name, input) in wanted {
            match self.sections.get_mut(&name) {
                Some(slot) => {
                    let changed = slot.input != input;
                    slot.input = input.clone();
                    slot.view.update(cx, |s, cx| {
                        s.stamp = stamp;
                        if changed {
                            s.input = input;
                            s.follow_pulse(&pulse, cx);
                            cx.notify();
                        }
                    });
                }
                None => {
                    let ws = cx.entity().downgrade();
                    let renders = self.section_renders.clone();
                    let (n, i) = (name.clone(), input.clone());
                    let view = cx.new(|cx| {
                        let mut s = Section { ws, name: n, input: i, stamp, renders, pulse: None };
                        s.follow_pulse(&pulse, cx);
                        s
                    });
                    self.sections.insert(name, SectionSlot { view, input });
                }
            }
        }
        // the root's own working indicators: the overview's cards, the switcher's rows
        let working = |i: Option<Indicator>| matches!(i, Some(Indicator::Working { .. }));
        let cards = self.overview_shown() && self.overview_cards().iter().any(|c| working(c.status));
        // the switcher lists blocks with their own status, which a tab's may hide
        let blocks = |m: &crate::remote::Model| {
            m.sessions.iter().flat_map(|s| &s.blocks).any(|b| working(status::indicator_of(&b.newest_first())))
        };
        let switcher = self.switcher_open() && self.hosts.remotes().any(|(_, r)| blocks(&r.model()));
        let root = cards || switcher;
        self.pulse_root = match (root, self.pulse_root.take()) {
            (true, None) => Some(cx.observe(&pulse, |_, _, cx| cx.notify())),
            (true, kept) => kept,
            (false, _) => None,
        };
        let wanted = root || self.sections.values().any(|s| s.input.working());
        self.pulse_wanted = wanted;
        pulse.update(cx, |p, cx| p.set_running(wanted, cx));
    }

    /// Draw a section's helper from its input.
    fn draw_section(&self, input: &Input, cx: &mut Context<Self>) -> gpui::AnyElement {
        match input {
            Input::Sidebar(i) => self.render_sidebar(i, cx).into_any_element(),
            Input::TitleBar(i) => self.render_titlebar(i, cx).into_any_element(),
            Input::Header(i) => self.render_header(i, cx).into_any_element(),
        }
    }

    /// Section `name`, cached in `style`; drawn directly from `input` when it has no view yet (the
    /// first frame, or a pane the model shows before the poll ran): never a view made while drawing.
    pub(super) fn section(
        &self,
        name: &str,
        style: StyleRefinement,
        input: impl FnOnce(&Self, &App) -> Option<Input>,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        if let Some(s) = self.sections.get(name) {
            return AnyView::from(s.view.clone()).cached(style).into_any_element();
        }
        match input(self, cx) {
            Some(input) => self.draw_section(&input, cx),
            None => div().into_any_element(),
        }
    }

    pub(super) fn sidebar_section(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let style = StyleRefinement::default().w(px(self.settings.sidebar_width)).h_full().flex_none();
        let input = |ws: &Self, _: &App| ws.sidebar_input(&ws.rows()).map(Input::Sidebar);
        self.section("sidebar", style, input, cx)
    }

    pub(super) fn titlebar_section(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let style = StyleRefinement::default().w_full().h(px(TITLEBAR)).flex_none();
        let input = |ws: &Self, _: &App| ws.titlebar_input(&ws.rows()).map(Input::TitleBar);
        self.section("titlebar", style, input, cx)
    }

    /// Pane `id`'s header in a pane `width` wide.
    pub(super) fn header_section(&self, id: &str, width: f32, cx: &mut Context<Self>) -> gpui::AnyElement {
        let style = StyleRefinement::default().w_full().h(px(panes::HEADER)).flex_none();
        let input = |ws: &Self, cx: &App| Some(Input::Header(ws.header_input(id, width, cx)));
        self.section(&format!("header-{id}"), style, input, cx)
    }

    /// Per section, how often it rendered (tests: `sidebar`, `titlebar`, `header-<block>`).
    pub fn section_renders(&self) -> HashMap<String, u64> {
        self.section_renders.borrow().clone()
    }

    /// The Pulse: whether it runs, and how often it ticked (tests).
    pub fn pulse_state(&self, cx: &App) -> (bool, u64) {
        let p = self.pulse.read(cx);
        (p.running(), p.ticks())
    }
}

use super::header::{HEADER_PARTS, TITLE_MAX};
