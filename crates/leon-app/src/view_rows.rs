//! Closing and renaming tabs and sessions: the palette's actions, the × on sidebar rows and
//! titlebar tabs, their right-click menus (`popover.rs`), and the inline rename a double-click
//! starts. A close asks first when a program other than a shell runs there, as quitting does; the
//! server's `session.close_window`, `session.destroy`, `session.set_window_label` and
//! `session.set_label` do the work, on the row's own machine.

use gpui::{Entity, KeyDownEvent, Subscription};

use super::super::hosts_view::Ask;
use super::super::*;
use crate::popover::{self, Item, Popover, PopoverEvent};
use crate::program_kind::{kind_of, Kind};

/// The longest name the inline field takes.
const NAME_MAX: usize = 200;

/// A sidebar row or titlebar tab: a session, or one of its tabs, on machine `host`.
#[derive(Clone, PartialEq, Debug)]
pub(in crate::view) struct RowRef {
    pub(in crate::view) host: usize,
    pub(in crate::view) session: String,
    pub(in crate::view) window: Option<String>,
}

/// An inline rename: the row, the text so far, and whether the old name is still all selected
/// (the first key typed replaces it).
#[derive(Clone, PartialEq, Debug)]
pub(in crate::view) struct Renaming {
    pub(in crate::view) row: RowRef,
    pub(in crate::view) text: String,
    pub(in crate::view) fresh: bool,
}

struct RowMenu {
    row: RowRef,
    popover: Entity<Popover>,
    _events: Subscription,
}

/// The rows' state, in `TermUi`.
#[derive(Default)]
pub(in crate::view) struct RowsUi {
    pub(in crate::view) renaming: Option<Renaming>,
    menu: Option<RowMenu>,
}

impl Workspace {
    /// The inline rename's text, while one is open.
    pub fn renaming(&self) -> Option<String> {
        self.term.rows.renaming.as_ref().map(|r| r.text.clone())
    }

    /// The open right-click menu of a row or tab.
    pub fn row_menu(&self) -> Option<Entity<Popover>> {
        self.term.rows.menu.as_ref().map(|m| m.popover.clone())
    }

    /// The programs other than shells in front in session `session` (in its tab `window` only, if
    /// given), on whichever machine has it: what a close asks about.
    pub fn running_programs(&self, session: &str, window: Option<&str>) -> Vec<String> {
        let Some(remote) = self.rows().iter().find(|r| r.id == session).and_then(|r| self.row_remote(r.host)) else {
            return Vec::new();
        };
        let model = remote.model();
        let Some(s) = model.session(session) else { return Vec::new() };
        let blocks = s.blocks.iter().filter(|b| window.is_none_or(|w| b.window_id == w));
        let names = blocks.filter_map(|b| b.foreground.clone().flatten());
        names.filter(|n| kind_of(n) != Kind::Shell).collect()
    }

    fn row_remote(&self, host: usize) -> Option<Rc<Remote>> {
        self.hosts.machines().get(host).and_then(|m| m.remote()).cloned()
    }

    /// The palette's Close Tab, Close Session, Rename Tab… and Rename Session…, on the session on
    /// screen and its tab.
    pub(in crate::view) fn row_action(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(session) = self.session_id.clone() else { return };
        let tab = self.app.model().session(&session).and_then(|s| s.windows.get(s.active).map(|w| w.id.clone()));
        let tabbed = name.starts_with("client.tab.");
        let row = RowRef { host: self.host, session, window: tab.filter(|_| tabbed) };
        if tabbed && row.window.is_none() {
            return;
        }
        match name.ends_with(".close") {
            true => self.close_row(row, window, cx),
            false => {
                let name = self.row_name(&row).unwrap_or_default();
                let ask = Some((Ask::Rename { row, name }, None));
                self.palette =
                    Some(Palette { query: String::new(), selected: 0, ask, switch: false, note: None, themes: None });
                cx.notify();
            }
        }
    }

    /// The name a row shows: its session's or its tab's title.
    fn row_name(&self, row: &RowRef) -> Option<String> {
        let s = self.rows().into_iter().find(|r| r.id == row.session)?;
        match &row.window {
            Some(w) => s.windows.into_iter().find(|t| &t.id == w).map(|t| t.title),
            None => Some(s.title),
        }
    }

    /// Close a tab or a session; with programs other than shells running there, after asking.
    pub(in crate::view) fn close_row(&mut self, row: RowRef, window: &mut Window, cx: &mut Context<Self>) {
        let running = self.running_programs(&row.session, row.window.as_deref());
        if running.is_empty() {
            return self.close_now(row, cx);
        }
        let what = if row.window.is_some() { "Tab" } else { "Session" };
        let (n, list) = (running.len(), running.join(", "));
        let detail = match n {
            1 => format!("{list} is running in it and stops."),
            _ => format!("{n} programs are running in it and stop: {list}."),
        };
        let buttons = [PromptButton::Ok(format!("Close {what}").into()), PromptButton::Cancel("Cancel".into())];
        let title = format!("Close this {}?", what.to_lowercase());
        let answer = window.prompt(PromptLevel::Warning, &title, Some(&detail), &buttons, cx);
        cx.spawn(async move |this, cx| {
            if answer.await == Ok(0) {
                let _ = this.update(cx, |ws, cx| ws.close_now(row, cx));
            }
        })
        .detach();
    }

    fn close_now(&mut self, row: RowRef, cx: &mut Context<Self>) {
        let Some(remote) = self.row_remote(row.host) else { return };
        match &row.window {
            Some(w) => {
                remote.act_later("session.close_window", json!({ "session_id": row.session, "window_id": w }), drop)
            }
            None => remote.act_later("session.destroy", json!({ "session_id": row.session }), drop),
        }
        let rows = self.rows();
        self.settle(&rows);
        cx.notify();
    }

    /// Set a row's name; an empty one clears its label, so it shows its program's title again.
    pub(in crate::view) fn rename_row(&mut self, row: RowRef, name: String, cx: &mut Context<Self>) {
        let Some(remote) = self.row_remote(row.host) else { return };
        let label = name.trim();
        match &row.window {
            Some(w) => remote.act_later(
                "session.set_window_label",
                json!({ "session_id": row.session, "window_id": w, "label": label }),
                drop,
            ),
            None => remote.act_later("session.set_label", json!({ "session_id": row.session, "label": label }), drop),
        }
        cx.notify();
    }

    /// Turn a row's name into a field holding it, all selected.
    pub(in crate::view) fn start_rename(&mut self, row: RowRef, cx: &mut Context<Self>) {
        if let Some(text) = self.row_name(&row) {
            self.term.rows.renaming = Some(Renaming { row, text, fresh: true });
            cx.notify();
        }
    }

    /// Set the inline rename's text and close the field (Enter, a click elsewhere).
    pub(in crate::view) fn rename_commit(&mut self, cx: &mut Context<Self>) {
        if let Some(r) = self.term.rows.renaming.take() {
            if !r.fresh {
                self.rename_row(r.row, r.text, cx);
            }
            cx.notify();
        }
    }

    /// A key while the inline field is open: it edits the name, and goes nowhere else.
    pub(in crate::view) fn rename_key(&mut self, ev: &KeyDownEvent, cx: &mut Context<Self>) {
        let ks = &ev.keystroke;
        let Some(r) = self.term.rows.renaming.as_mut() else { return };
        match ks.key.as_str() {
            "enter" => return self.rename_commit(cx),
            "escape" => self.term.rows.renaming = None,
            "backspace" if r.fresh => (r.text, r.fresh) = (String::new(), false),
            "backspace" => drop(r.text.pop()),
            _ if ks.modifiers.platform || ks.modifiers.control => return,
            _ => {
                let Some(typed) = ks.key_char.as_ref().filter(|c| !c.chars().any(char::is_control)) else { return };
                if std::mem::take(&mut r.fresh) {
                    r.text.clear();
                }
                if r.text.chars().count() + typed.chars().count() <= NAME_MAX {
                    r.text.push_str(typed);
                }
            }
        }
        cx.notify();
    }

    /// The inline field in a row's place.
    pub(in crate::view) fn rename_field(&self, r: &Renaming, t: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let text = div().when(r.fresh, |d| d.bg(rgb(t.selection))).child(r.text.clone());
        div()
            .debug_selector(|| sel("rename-field".into()))
            .flex()
            .flex_1()
            .min_w(px(0.))
            .overflow_hidden()
            .px(px(4.))
            .rounded(px(4.))
            .border_1()
            .border_color(rgb(t.accent))
            .bg(rgb(t.bg))
            .text_color(rgb(t.title))
            .child(text)
            .when(!r.fresh, |d| d.child(div().w(px(1.)).h(px(14.)).bg(rgb(t.title))))
            .on_mouse_down_out(cx.listener(|ws, _, _, cx| ws.rename_commit(cx)))
    }

    /// A right-click on a row or tab: its menu at the mouse, for the keyboard too.
    pub(in crate::view) fn open_row_menu(
        &mut self,
        row: RowRef,
        at: gpui::Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let items = match row.window {
            Some(_) => vec![Item::new("rename", "Rename Tab…", None), Item::new("close", "Close Tab", None)],
            None => vec![
                Item::new("new_tab", "New Tab", None),
                Item::new("rename", "Rename Session…", None),
                Item::new("close", "Close Session", None),
            ],
        };
        let anchor = gpui::Bounds { origin: at, size: gpui::size(px(0.), px(0.)) };
        let theme = self.theme;
        let p = cx.new(|cx| Popover::new(anchor, Vec::new(), items, theme, window, cx));
        let events =
            cx.subscribe_in(&p, window, |ws, _, ev: &PopoverEvent, window, cx| ws.row_menu_event(ev, window, cx));
        p.update(cx, |p, cx| p.focus(window, cx));
        self.term.rows.menu = Some(RowMenu { row, popover: p, _events: events });
        // the root takes the focus on a press it isn't told to leave (gpui's div.rs, `track_focus`)
        window.prevent_default();
        cx.stop_propagation();
        cx.notify();
    }

    fn row_menu_event(&mut self, ev: &PopoverEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(menu) = self.term.rows.menu.take() else { return };
        window.focus(&self.focus);
        cx.notify();
        let PopoverEvent::Chose(what) = ev else { return };
        match what.as_str() {
            "new_tab" => {
                self.select(&menu.row.session, None, cx);
                self.new_tab(&NewTab, window, cx);
            }
            "rename" => self.start_rename(menu.row, cx),
            "close" => self.close_row(menu.row, window, cx),
            _ => {}
        }
    }

    /// A row's × (shown while the mouse is on `group`): closes it by `close_row`'s rule.
    pub(in crate::view) fn close_button(
        &self,
        name: String,
        group: String,
        row: RowRef,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let selector = name.clone();
        div()
            .id(SharedString::from(name.clone()))
            .debug_selector(move || sel(selector))
            .group(SharedString::from(name.clone()))
            .px(px(4.))
            .rounded(px(4.))
            .invisible()
            .group_hover(SharedString::from(group), |d| d.visible())
            .hover(|d| d.bg(rgb(t.hover)).text_color(rgb(t.title)))
            .child(Icon::Close.in_button(ROW_ICON, t.muted, t.title, name))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |ws, _, window, cx| {
                    cx.stop_propagation();
                    ws.close_row(row.clone(), window, cx);
                }),
            )
    }

    /// A row's name, or the inline field when it is being renamed.
    pub(in crate::view) fn name_or_field(
        &self,
        renaming: Option<&Renaming>,
        row: &RowRef,
        name: String,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        match renaming.filter(|r| &r.row == row) {
            Some(r) => self.rename_field(r, t, cx).into_any_element(),
            None => div()
                .flex_1()
                .overflow_hidden()
                .min_w(px(0.))
                .line_clamp(1)
                .text_ellipsis()
                .child(name)
                .into_any_element(),
        }
    }

    /// The open row menu, over everything.
    pub(in crate::view) fn render_row_menu(&self, cx: &App) -> Option<impl IntoElement> {
        self.term.rows.menu.as_ref().map(|m| popover::show(&m.popover, cx))
    }
}
