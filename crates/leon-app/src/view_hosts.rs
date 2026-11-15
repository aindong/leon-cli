//! The window's machines: which host the selection is on, each host's label in the sidebar (a dot
//! for its state, a note while it isn't connected), the stale overlay over a reconnecting host's
//! panes, New Session on a host, and the Add Host prompt in the palette.

use super::*;
use crate::hosts::Machine;
use crate::palette::Item;

/// What the palette asks for.
#[derive(Clone)]
pub(super) enum Ask {
    HostLabel,
    HostEndpoint { label: String },
}

/// A machine as the sidebar shows it.
#[derive(Debug, Clone)]
pub struct HostRow {
    pub label: String,
    pub state: HostState,
    /// What the sidebar says under the label (`unreachable: …`), while not connected.
    pub note: Option<String>,
    pub collapsed: bool,
    pub sessions: Vec<HostSession>,
}

#[derive(Debug, Clone)]
pub struct HostSession {
    pub id: String,
    pub title: String,
    pub tabs: Vec<String>,
}

impl Workspace {
    /// This Mac and each host, with their state and sessions.
    pub fn host_rows(&self) -> Vec<HostRow> {
        let rows = self.rows();
        let sessions = |h: usize| {
            let mine = rows.iter().filter(|r| r.host == h);
            let tabs = |r: &SessionRow| r.windows.iter().map(|w| w.title.clone()).collect();
            mine.map(|r| HostSession { id: r.id.clone(), title: r.title.clone(), tabs: tabs(r) }).collect()
        };
        let machines = self.hosts.machines().iter().enumerate();
        machines
            .map(|(h, m)| {
                let state = m.state();
                HostRow {
                    note: state.note(&m.label),
                    state,
                    collapsed: self.host_collapsed(&m.label),
                    sessions: sessions(h),
                    label: m.label.clone(),
                }
            })
            .collect()
    }

    /// A host's remote, while it is connected or reconnecting.
    pub fn host_remote(&self, label: &str) -> Option<Rc<Remote>> {
        self.hosts.machines().get(self.hosts.index_of(label)?)?.remote().cloned()
    }

    /// The host a pane on screen is on, when it isn't This Mac (its header names it).
    pub fn pane_host(&self, block_id: &str) -> Option<String> {
        let m = &self.hosts.machines()[self.host];
        (!m.is_local() && self.app.model().block(block_id).is_some()).then(|| m.label.clone())
    }

    /// Whether the sidebar groups sessions under their machines: only once there is a host.
    pub(super) fn grouped(&self) -> bool {
        self.hosts.machines().len() > 1
    }

    pub(super) fn host_collapsed(&self, label: &str) -> bool {
        self.settings.collapsed_hosts.iter().any(|l| l == label)
    }

    /// Collapse or expand a machine's group; kept for the next launch.
    pub(super) fn toggle_host(&mut self, label: &str, cx: &mut Context<Self>) {
        let c = &mut self.settings.collapsed_hosts;
        match c.iter().position(|l| l == label) {
            Some(i) => drop(c.remove(i)),
            None => c.push(label.to_string()),
        }
        self.save_settings();
        cx.notify();
    }

    /// Act on machine `i` from now on; the panes of the one left stop streaming.
    pub(super) fn use_host(&mut self, i: usize) {
        let Some(remote) = self.hosts.machines().get(i).and_then(Machine::remote).cloned() else { return };
        if i != self.host {
            self.app.watch(Vec::new());
            (self.host, self.app) = (i, remote);
        }
    }

    /// The poll's share: pick up hosts that connected or gave up, and select a session made over
    /// a link once its host's model has it.
    pub(super) fn poll_hosts(&mut self) {
        let changed = self.hosts.poll();
        if changed {
            self.send_palette(); // a host that connected gets the theme's colours
        }
        if changed && !self.hosts.machines()[self.host].remote().is_some_and(|r| Rc::ptr_eq(r, &self.app)) {
            self.use_host(0); // the selected host went away
        }
        let later = lock(&self.select_later).clone();
        if let Some((host, sid)) = later {
            let there = self.hosts.machines().get(host).and_then(Machine::remote);
            if there.is_some_and(|r| r.model().session(&sid).is_some()) {
                *lock(&self.select_later) = None;
                self.use_host(host);
                self.session_id = Some(sid);
            }
        }
    }

    /// Create a session on machine `host` (This Mac when unknown) and select it, at once on This
    /// Mac, once it shows over a link.
    pub(super) fn new_session_on(&mut self, host: usize, cx: &mut Context<Self>) {
        let Some(remote) = self.hosts.machines().get(host).and_then(Machine::remote).cloned() else { return };
        let later = self.select_later.clone();
        remote.act_later("session.create", json!({}), move |r| {
            if let Some(sid) = r.ok().and_then(|r| r["session_id"].as_str().map(String::from)) {
                *lock(&later) = Some((host, sid));
            }
        });
        self.poll_hosts();
        cx.notify();
    }

    /// The palette's machine rows: New Session on each connected machine (when there are hosts).
    pub(super) fn host_items(&self) -> Vec<Item> {
        if !self.grouped() {
            return Vec::new();
        }
        let on = self.hosts.remotes().map(|(i, _)| self.hosts.machines()[i].label.clone());
        on.map(|label| Item {
            name: "session.new".into(),
            title: format!("New Session on {label}"),
            keywords: vec![label.clone()],
            shortcut: None,
            args: Some(json!({ "host": label })),
        })
        .collect()
    }

    /// A machine's label: a dot for its state, the name, and under it a note while not connected.
    pub(super) fn host_label(&self, h: usize, m: &Machine, cx: &mut Context<Self>) -> impl IntoElement {
        let t = &self.theme;
        let state = m.state();
        let dot = match state {
            HostState::Connected => t.done,
            HostState::Connecting | HostState::Reconnecting { .. } => t.working,
            HostState::NeedsLogin(_) => t.blocked,
            HostState::Unreachable(_) => t.error,
        };
        let label = m.label.clone();
        let collapsed = self.host_collapsed(&label);
        // collapsed, the label counts what it hides
        let sessions = m.remote().map(|r| r.model().sessions.len()).filter(|n| *n > 0);
        div()
            .flex()
            .flex_col()
            .flex_none()
            .mt(px(if h == 0 { 2. } else { 10. }))
            .child(
                div()
                    .id(SharedString::from(format!("host-{h}")))
                    .debug_selector(|| sel(format!("host-{h}")))
                    .flex()
                    .items_center()
                    .gap(px(7.))
                    .h(px(24.))
                    .px(px(6.))
                    .rounded(px(5.))
                    .cursor_pointer()
                    .hover(|d| d.bg(rgb(t.hover)))
                    .text_size(px(SECONDARY_TEXT))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(t.muted))
                    .child(
                        div()
                            .debug_selector(|| sel(format!("host-dot-{h}")))
                            .flex_none()
                            .size(px(7.))
                            .rounded_full()
                            .bg(rgb(dot)),
                    )
                    .child(div().flex_1().min_w(px(0.)).line_clamp(1).text_ellipsis().child(label.clone()))
                    .when(collapsed, |d| {
                        d.children(sessions.map(|n| div().font_weight(FontWeight::NORMAL).child(n.to_string())))
                    })
                    .on_mouse_down(MouseButton::Left, cx.listener(move |ws, _, _, cx| ws.toggle_host(&label, cx))),
            )
            .children(state.note(&m.label).map(|note| {
                div()
                    .debug_selector(|| sel(format!("host-note-{h}")))
                    .pl(px(20.))
                    .pr(px(6.))
                    .pb(px(2.))
                    .text_size(px(SECONDARY_TEXT))
                    .text_color(rgb(if matches!(state, HostState::Unreachable(_)) { t.error } else { t.muted }))
                    .line_clamp(3)
                    .text_ellipsis()
                    .child(note)
            }))
    }

    /// Over the panes of a host whose link is down: they show what they last showed, dimmed, with
    /// the reconnect's progress.
    pub(super) fn render_stale(&self, window: &Window) -> Option<impl IntoElement> {
        let m = &self.hosts.machines()[self.host];
        let note = match m.state() {
            s @ HostState::Reconnecting { .. } => s.note(&m.label)?,
            _ => return None,
        };
        self.shown()?;
        let t = &self.theme;
        // the pane area is the window's bottom right corner (`pane_area`)
        let (w, h) = self.pane_area(window);
        let view = window.viewport_size();
        let (left, top) = (f32::from(view.width) - w, f32::from(view.height) - h);
        Some(
            div()
                .debug_selector(|| sel("stale-overlay".into()))
                .absolute()
                .left(px(left))
                .top(px(top))
                .w(px(w))
                .h(px(h))
                .flex()
                .items_end()
                .justify_center()
                .pb(px(28.))
                .bg(gpui::Hsla::from(rgb(t.chrome)).opacity(0.6))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .px(px(14.))
                        .py(px(8.))
                        .rounded(px(8.))
                        .bg(rgb(t.chrome))
                        .border_1()
                        .border_color(rgb(t.border))
                        .text_color(rgb(t.text))
                        .child(div().size(px(7.)).rounded_full().bg(rgb(t.working)))
                        .child(format!("{}: {note}", m.label)),
                ),
        )
    }

    /// Open the palette asking for a new host's label.
    pub(super) fn ask_host(&mut self, cx: &mut Context<Self>) {
        self.palette =
            Some(Palette { query: String::new(), selected: 0, ask: Some((Ask::HostLabel, None)), switch: false });
        cx.notify();
    }

    /// The prompt's text and its one row (what to type, or why the last try was refused).
    pub(super) fn ask_rows(&self) -> Option<(String, String)> {
        let (ask, refused) = self.palette.as_ref()?.ask.as_ref()?;
        let (prompt, hint) = match ask {
            Ask::HostLabel => ("Host label…", "A label for the host: letters, digits, . _ - (devbox)".to_string()),
            Ask::HostEndpoint { label } => {
                ("ssh://[user@]host[:port]…", format!("How ssh reaches {label}: ssh://me@devbox.example.com"))
            }
        };
        Some((prompt.to_string(), refused.clone().unwrap_or(hint)))
    }

    /// Enter in the prompt: check what was typed as `leon hosts add` does, then ask the next
    /// thing, or save the host and start connecting to it.
    pub(super) fn ask_enter(&mut self, cx: &mut Context<Self>) {
        let Some(p) = self.palette.as_mut() else { return };
        let Some((ask, refused)) = p.ask.as_mut() else { return };
        let typed = p.query.trim().to_string();
        let mut list = match leon_hosts::hosts::load() {
            Ok(l) => l,
            Err(e) => {
                *refused = Some(e);
                return cx.notify();
            }
        };
        match ask.clone() {
            // one in the window but no longer in the file (removed from the CLI) is taken too
            Ask::HostLabel if self.hosts.index_of(&typed).is_some() => {
                *refused = Some(format!("{} {typed:?} already", leon_hosts::hosts::TAKEN));
            }
            Ask::HostLabel => match leon_hosts::hosts::check_new_label(&list, &typed) {
                Ok(()) => {
                    (*ask, *refused) = (Ask::HostEndpoint { label: typed }, None);
                    p.query.clear();
                }
                Err(e) => *refused = Some(e),
            },
            Ask::HostEndpoint { label } => {
                let host = leon_hosts::Host { label, endpoints: vec![typed], icon: None };
                let saved =
                    leon_hosts::hosts::add(&mut list, host.clone()).and_then(|()| leon_hosts::hosts::save(&list));
                match saved {
                    Ok(()) => {
                        self.palette = None;
                        self.hosts.add(host);
                    }
                    Err(e) => *refused = Some(e),
                }
            }
        }
        cx.notify();
    }

    /// The machine a `session.new` names (`{"host": LABEL}`), else the selected session's.
    pub(super) fn host_arg(&self, args: Option<&Value>) -> usize {
        let named = args.and_then(|a| a["host"].as_str()).and_then(|l| self.hosts.index_of(l));
        named.unwrap_or(self.host)
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}
