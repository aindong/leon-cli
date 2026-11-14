//! The window: a sidebar of sessions, a tab bar of the selected session's windows, and the active
//! window's split layout, each pane a terminal painted from the in-process grid. A poll on the
//! GPUI executor redraws only when a visible terminal or the session model changed.

use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use alacritty_terminal::term::TermMode;
use alacritty_terminal::vte::ansi::CursorShape;
use gpui::{
    actions, canvas, div, fill, font, outline, point, prelude::*, px, rgb, size, App, Application, Bounds, Context,
    CursorStyle, FocusHandle, Font, FontStyle, FontWeight, KeyBinding, KeyDownEvent, Menu, MenuItem, MouseButton,
    MouseMoveEvent, MouseUpEvent, Pixels, SharedString, StrikethroughStyle, Task, TextRun, TitlebarOptions,
    UnderlineStyle, Window, WindowBounds, WindowOptions,
};
use leon_core::layout::Direction;
use leon_core::terminal::Terminal;
use serde_json::json;

use crate::embedded::Embedded;
use crate::panes::{self, Divider};
use crate::render::{self, Frame};
use crate::theme::{Theme, DARK};

actions!(
    leon,
    [
        NewTab,
        ClosePane,
        NewSession,
        NextTab,
        PreviousTab,
        Paste,
        ToggleSidebar,
        Quit,
        SplitRight,
        SplitDown,
        FocusLeft,
        FocusRight,
        FocusUp,
        FocusDown,
        ToggleZoom
    ]
);

const FONT_FAMILY: &str = "Menlo";
const FONT_SIZE: f32 = 13.;
const LINE_HEIGHT: f32 = 1.3;
const SIDEBAR_WIDTH: f32 = 220.;
const TAB_BAR_HEIGHT: f32 = 38.;
/// Space between a pane's edge and its text.
const PADDING: f32 = 8.;
/// The divider line between panes, and how far either side of it a drag can start.
const DIVIDER: f32 = 1.;
const DIVIDER_GRAB: f32 = 3.;
/// How often the poll looks for changes (about 120 Hz).
const POLL: Duration = Duration::from_millis(8);

/// The block input goes to: the one on screen when a block is zoomed, else the focused one. The
/// model keeps zoom and focus apart (a CLI can zoom any block), so this is what the user sees.
fn target(w: &leon_core::Window) -> Option<&str> {
    w.zoomed.as_deref().or(w.focus.as_deref())
}

/// Key bindings. Cmd+1..9 are handled in `key_down`, so one rule covers them all.
pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-t", NewTab, None),
        KeyBinding::new("cmd-w", ClosePane, None),
        KeyBinding::new("cmd-d", SplitRight, None),
        KeyBinding::new("cmd-shift-d", SplitDown, None),
        KeyBinding::new("cmd-alt-left", FocusLeft, None),
        KeyBinding::new("cmd-alt-right", FocusRight, None),
        KeyBinding::new("cmd-alt-up", FocusUp, None),
        KeyBinding::new("cmd-alt-down", FocusDown, None),
        KeyBinding::new("cmd-shift-enter", ToggleZoom, None),
        KeyBinding::new("cmd-n", NewSession, None),
        KeyBinding::new("cmd-shift-]", NextTab, None),
        KeyBinding::new("cmd-}", NextTab, None),
        KeyBinding::new("cmd-shift-[", PreviousTab, None),
        KeyBinding::new("cmd-{", PreviousTab, None),
        KeyBinding::new("cmd-v", Paste, None),
        KeyBinding::new("cmd-b", ToggleSidebar, None),
        KeyBinding::new("cmd-q", Quit, None),
    ]);
}

/// Open the app window on `server` and run until it quits.
pub fn run(server: Embedded) {
    let server = Rc::new(server);
    Application::new().run(move |cx: &mut App| {
        init(cx);
        cx.set_menus(vec![
            Menu { name: "leon".into(), items: vec![MenuItem::action("Quit leon", Quit)] },
            Menu {
                name: "Shell".into(),
                items: vec![
                    MenuItem::action("New Tab", NewTab),
                    MenuItem::action("New Session", NewSession),
                    MenuItem::separator(),
                    MenuItem::action("Split Right", SplitRight),
                    MenuItem::action("Split Down", SplitDown),
                    MenuItem::action("Zoom Pane", ToggleZoom),
                    MenuItem::separator(),
                    MenuItem::action("Close Pane", ClosePane),
                ],
            },
            Menu { name: "Edit".into(), items: vec![MenuItem::action("Paste", Paste)] },
            Menu {
                name: "View".into(),
                items: vec![
                    MenuItem::action("Toggle Sidebar", ToggleSidebar),
                    MenuItem::separator(),
                    MenuItem::action("Next Tab", NextTab),
                    MenuItem::action("Previous Tab", PreviousTab),
                ],
            },
        ]);
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.on_window_closed(|cx| cx.quit()).detach();
        // However the app quits (menu, Dock, logout), end the programs and remove the socket.
        let on_quit = server.clone();
        cx.on_app_quit(move |_| {
            on_quit.stop();
            async {}
        })
        .detach();
        let bounds = Bounds::centered(None, size(px(1100.), px(700.)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some("leon".into()),
                appears_transparent: true,
                traffic_light_position: Some(point(px(14.), px(13.))),
            }),
            window_min_size: Some(size(px(480.), px(260.))),
            ..WindowOptions::default()
        };
        let app = server.clone();
        cx.open_window(options, move |window, cx| cx.new(|cx| Workspace::new(app, window, cx)))
            .expect("open the leon window");
        cx.activate(true);
    });
}

/// Frame build times (render start to paint end), printed each second that drew when
/// `LEON_FRAME_STATS=1`.
#[derive(Default)]
struct Stats {
    enabled: bool,
    render_start: Option<Instant>,
    frames: u32,
    total: Duration,
    max: Duration,
    since: Option<Instant>,
}

impl Stats {
    fn painted(&mut self) {
        if let Some(start) = self.render_start.take() {
            let took = start.elapsed();
            self.frames += 1;
            self.total += took;
            self.max = self.max.max(took);
            self.since.get_or_insert_with(Instant::now);
        }
    }

    fn report(&mut self) {
        let Some(since) = self.since.filter(|s| s.elapsed() >= Duration::from_secs(1)) else { return };
        let ms = |d: Duration| d.as_secs_f64() * 1000.;
        eprintln!(
            "leon frames: {} in {:.2}s, avg {:.2} ms, max {:.2} ms",
            self.frames,
            since.elapsed().as_secs_f64(),
            ms(self.total) / self.frames as f64,
            ms(self.max)
        );
        (self.frames, self.total, self.max, self.since) = (0, Duration::ZERO, Duration::ZERO, None);
    }
}

/// What one render needs from the model, copied out under the state lock.
struct SessionRow {
    id: String,
    title: String,
    windows: Vec<WindowRow>,
    active: usize,
}

struct WindowRow {
    id: String,
    title: String,
}

/// The terminals on screen, by block id, in leaf order.
type Panes = Vec<(String, Arc<Terminal>)>;

/// A divider being dragged: the split it sets, and where the drag started.
struct Drag {
    divider: Divider,
    from: Pixels,
    /// The ratio last sent, so a move that changes nothing sends nothing.
    last: Option<f64>,
}

pub struct Workspace {
    app: Rc<Embedded>,
    drag: Option<Drag>,
    session_id: Option<String>,
    /// Where the selected session sat in the list when last seen.
    selected_at: usize,
    sidebar: bool,
    collapsed: HashSet<String>,
    focus: FocusHandle,
    stats: Rc<RefCell<Stats>>,
    /// Hash of everything drawn last time; the poll redraws when it moves.
    seen: u64,
    _poll: Task<()>,
}

impl Workspace {
    pub fn new(app: Rc<Embedded>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        if app.ctx().state().sessions.is_empty() {
            let _ = app.call("session.create", json!({}));
        }
        let focus = cx.focus_handle();
        window.focus(&focus);
        let poll = cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(POLL).await;
            let alive = this.update(cx, |ws, cx| {
                if ws.app.stopped() {
                    cx.quit(); // `leon server stop` from outside: app and server are one process
                    return;
                }
                ws.stats.borrow_mut().report();
                let now = ws.signature();
                if now != ws.seen {
                    ws.seen = now;
                    cx.notify();
                }
            });
            if alive.is_err() {
                return;
            }
        });
        let stats = Stats { enabled: std::env::var("LEON_FRAME_STATS").is_ok_and(|v| v == "1"), ..Stats::default() };
        let session_id = app.ctx().state().sessions.first().map(|s| s.id.clone());
        Workspace {
            app,
            drag: None,
            session_id,
            selected_at: 0,
            sidebar: true,
            collapsed: HashSet::new(),
            focus,
            stats: Rc::new(RefCell::new(stats)),
            seen: 0,
            _poll: poll,
        }
    }

    /// The selected session, if any.
    pub fn session_id(&self) -> Option<String> {
        self.session_id.clone()
    }

    pub fn sidebar_visible(&self) -> bool {
        self.sidebar
    }

    /// The terminal input goes to (`target`) in the selected session's active window.
    pub fn terminal(&self) -> Option<Arc<Terminal>> {
        let (_, id, _) = self.target()?;
        Some(self.app.ctx().state().block(&id).ok()?.1.terminal.clone())
    }

    /// (session id, target block id, whether it is zoomed) of the selected session's active window.
    fn target(&self) -> Option<(String, String, bool)> {
        let sid = self.session_id.clone()?;
        let st = self.app.ctx().state();
        let s = st.session(&sid).ok()?;
        let w = s.windows.get(s.active)?;
        Some((sid, target(w)?.to_string(), w.zoomed.is_some()))
    }

    /// The active window's layout and the terminals it shows, in leaf order.
    fn shown(&self) -> Option<(leon_core::Window, Panes)> {
        let st = self.app.ctx().state();
        let s = st.session(self.session_id.as_deref()?).ok()?;
        let w = s.windows.get(s.active)?;
        let ids: Vec<&str> = match &w.zoomed {
            Some(z) => vec![z.as_str()],
            None => w.layout.as_ref()?.leaves(),
        };
        let terms = ids
            .into_iter()
            .filter_map(|id| s.blocks.iter().find(|b| b.id == id))
            .map(|b| (b.id.clone(), b.terminal.clone()))
            .collect();
        Some((w.clone(), terms))
    }

    /// Keep the selection on a live session: one that ended (closed here, its shell exited, or
    /// killed from the CLI) hands over to the session now in its place.
    fn settle(&mut self, rows: &[SessionRow]) {
        if let Some(i) = rows.iter().position(|r| Some(&r.id) == self.session_id.as_ref()) {
            self.selected_at = i;
            return;
        }
        self.selected_at = self.selected_at.min(rows.len().saturating_sub(1));
        self.session_id = rows.get(self.selected_at).map(|r| r.id.clone());
    }

    fn rows(&self) -> Vec<SessionRow> {
        let st = self.app.ctx().state();
        st.sessions
            .iter()
            .enumerate()
            .map(|(i, s)| SessionRow {
                id: s.id.clone(),
                title: s.label.clone().unwrap_or_else(|| format!("Session {}", i + 1)),
                windows: s
                    .windows
                    .iter()
                    .map(|w| WindowRow {
                        id: w.id.clone(),
                        title: w.label.clone().unwrap_or_else(|| {
                            let shown = target(w).and_then(|id| s.blocks.iter().find(|b| b.id == id));
                            shown.map_or("empty".into(), |b| program_name(&b.terminal.command))
                        }),
                    })
                    .collect(),
                active: s.active,
            })
            .collect()
    }

    /// Changes worth a redraw: every session's view (its revision moves with layout, focus, zoom
    /// and labels) and the content of every pane on screen. Hidden tabs and panes hidden by a zoom
    /// don't count.
    pub fn signature(&self) -> u64 {
        let mut h = DefaultHasher::new();
        {
            let st = self.app.ctx().state();
            for s in &st.sessions {
                (&s.id, &s.label, s.active, s.revision).hash(&mut h);
                for w in &s.windows {
                    (&w.id, &w.label).hash(&mut h);
                }
            }
        }
        self.session_id.hash(&mut h);
        for (id, t) in self.shown().map(|(_, terms)| terms).unwrap_or_default() {
            (id, Arc::as_ptr(&t) as usize, t.generation()).hash(&mut h);
        }
        h.finish()
    }

    /// Run a layout method on the selected session; redraw.
    fn call(&mut self, method: &str, mut params: serde_json::Value, cx: &mut Context<Self>) {
        let Some(sid) = self.session_id.clone() else { return };
        params["session_id"] = json!(sid);
        let _ = self.app.call(method, params);
        let rows = self.rows();
        self.settle(&rows);
        cx.notify();
    }

    /// Split the target pane; a zoom ends first so the new pane shows.
    fn split(&mut self, direction: Direction, cx: &mut Context<Self>) {
        let Some((_, id, zoomed)) = self.target() else { return };
        if zoomed {
            self.call("session.zoom_block", json!({ "block_id": id, "zoom": false }), cx);
        }
        let p = json!({ "anchor_id": id, "direction": direction, "side": "after", "layout": { "block": {} }, "focus": true });
        self.call("session.new_split", p, cx);
    }

    /// Move focus by direction from the pane on screen; a zoom ends first so the result shows.
    fn focus_toward(&mut self, toward: &str, cx: &mut Context<Self>) {
        let Some((_, id, zoomed)) = self.target() else { return };
        if zoomed {
            self.call("session.focus_block", json!({ "block_id": id }), cx);
            self.call("session.zoom_block", json!({ "block_id": id, "zoom": false }), cx);
        }
        self.call("session.focus_direction", json!({ "direction": toward }), cx);
    }

    fn split_right(&mut self, _: &SplitRight, _: &mut Window, cx: &mut Context<Self>) {
        self.split(Direction::Horizontal, cx);
    }

    fn split_down(&mut self, _: &SplitDown, _: &mut Window, cx: &mut Context<Self>) {
        self.split(Direction::Vertical, cx);
    }

    fn focus_left(&mut self, _: &FocusLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.focus_toward("left", cx);
    }

    fn focus_right(&mut self, _: &FocusRight, _: &mut Window, cx: &mut Context<Self>) {
        self.focus_toward("right", cx);
    }

    fn focus_up(&mut self, _: &FocusUp, _: &mut Window, cx: &mut Context<Self>) {
        self.focus_toward("up", cx);
    }

    fn focus_down(&mut self, _: &FocusDown, _: &mut Window, cx: &mut Context<Self>) {
        self.focus_toward("down", cx);
    }

    fn toggle_zoom(&mut self, _: &ToggleZoom, _: &mut Window, cx: &mut Context<Self>) {
        if let Some((_, id, _)) = self.target() {
            self.call("session.zoom_block", json!({ "block_id": id }), cx);
        }
    }

    /// Close the target pane; its window closes with its last pane, and its session with its last window.
    fn close_pane(&mut self, _: &ClosePane, _: &mut Window, cx: &mut Context<Self>) {
        if let Some((_, id, _)) = self.target() {
            self.call("block.close", json!({ "block_id": id }), cx);
        }
    }

    fn focus_block(&mut self, id: &str, cx: &mut Context<Self>) {
        self.call("session.focus_block", json!({ "block_id": id }), cx);
    }

    fn start_drag(&mut self, divider: Divider, at: gpui::Point<Pixels>, cx: &mut Context<Self>) {
        let from = if divider.direction == Direction::Horizontal { at.x } else { at.y };
        self.drag = Some(Drag { divider, from, last: None });
        cx.notify(); // the next frame installs the window-wide drag listeners
        cx.stop_propagation();
    }

    /// While dragging, the split's ratio follows the mouse: the start ratio plus the distance moved
    /// over the room the split shares out.
    fn drag_to(&mut self, ev: &MouseMoveEvent, cx: &mut Context<Self>) {
        let Some(drag) = &self.drag else { return };
        if ev.pressed_button != Some(MouseButton::Left) {
            self.drag = None;
            return;
        }
        let d = &drag.divider;
        let (at, room) = match d.direction {
            Direction::Horizontal => (ev.position.x, d.split.w - DIVIDER),
            Direction::Vertical => (ev.position.y, d.split.h - DIVIDER),
        };
        let moved = f32::from(at - drag.from) as f64;
        // Keep a ratio set elsewhere (say 0.01 from the CLI) where it is until the drag moves it.
        let ratio = (d.ratio + moved / room.max(1.) as f64).clamp(d.ratio.min(0.05), d.ratio.max(0.95));
        if drag.last == Some(ratio) {
            return; // each call is a layout change with events for every subscriber
        }
        let p =
            json!({ "direction": d.direction, "before_block_id": d.before, "after_block_id": d.after, "ratio": ratio });
        if let Some(d) = self.drag.as_mut() {
            d.last = Some(ratio);
        }
        self.call("session.resize_pane", p, cx);
    }

    fn end_drag(&mut self) {
        self.drag = None;
    }

    /// While a divider is dragged, follow the mouse anywhere in the window: element listeners only
    /// hear the mouse over their own unblocked area, and the divider itself blocks it.
    fn drag_listener(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let ws = cx.entity().downgrade();
        let dragging = self.drag.is_some();
        canvas(
            |_, _, _| {},
            move |_, _, window, _| {
                if !dragging {
                    return;
                }
                let moved = ws.clone();
                window.on_mouse_event(move |ev: &MouseMoveEvent, phase, _, cx| {
                    if phase == gpui::DispatchPhase::Capture {
                        moved.update(cx, |ws, cx| ws.drag_to(ev, cx)).ok();
                    }
                });
                let up = ws.clone();
                window.on_mouse_event(move |_: &MouseUpEvent, phase, _, cx| {
                    if phase == gpui::DispatchPhase::Capture {
                        up.update(cx, |ws, _| ws.end_drag()).ok();
                    }
                });
            },
        )
        .absolute()
        .size_0()
    }

    /// (session id, window id) of the selected session's window `index`, or of its active one.
    fn window_ref(&self, index: Option<usize>) -> Option<(String, String)> {
        let sid = self.session_id.clone()?;
        let st = self.app.ctx().state();
        let s = st.session(&sid).ok()?;
        let wid = s.windows.get(index.unwrap_or(s.active))?.id.clone();
        Some((sid, wid))
    }

    fn focus_window(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some((sid, wid)) = self.window_ref(Some(index)) {
            let _ = self.app.call("session.focus_window", json!({ "session_id": sid, "window_id": wid }));
            cx.notify();
        }
    }

    /// Close a window; if that ended its session, select a neighbouring one.
    fn close_window(&mut self, (sid, wid): (String, String), cx: &mut Context<Self>) {
        let _ = self.app.call("session.close_window", json!({ "session_id": sid, "window_id": wid }));
        let rows = self.rows();
        self.settle(&rows);
        cx.notify();
    }

    fn step_tab(&mut self, by: isize, cx: &mut Context<Self>) {
        let Some(sid) = self.session_id.clone() else { return };
        let Some((n, active)) = self.app.ctx().state().session(&sid).ok().map(|s| (s.windows.len(), s.active)) else {
            return;
        };
        if n > 0 {
            self.focus_window((active as isize + by).rem_euclid(n as isize) as usize, cx);
        }
    }

    fn new_tab(&mut self, _: &NewTab, window: &mut Window, cx: &mut Context<Self>) {
        match self.session_id.clone() {
            Some(sid) => drop(self.app.call("session.new_window", json!({ "session_id": sid }))),
            None => self.new_session(&NewSession, window, cx),
        }
        cx.notify();
    }

    fn new_session(&mut self, _: &NewSession, _: &mut Window, cx: &mut Context<Self>) {
        if let Ok(r) = self.app.call("session.create", json!({})) {
            self.session_id = r["session_id"].as_str().map(String::from);
        }
        cx.notify();
    }

    fn next_tab(&mut self, _: &NextTab, _: &mut Window, cx: &mut Context<Self>) {
        self.step_tab(1, cx);
    }

    fn previous_tab(&mut self, _: &PreviousTab, _: &mut Window, cx: &mut Context<Self>) {
        self.step_tab(-1, cx);
    }

    fn paste(&mut self, _: &Paste, _: &mut Window, cx: &mut Context<Self>) {
        let (Some(t), Some(text)) = (self.terminal(), cx.read_from_clipboard().and_then(|c| c.text())) else {
            return;
        };
        let bracketed = t.with_term(|term| term.mode().contains(TermMode::BRACKETED_PASTE));
        let _ = t.write(&crate::input::paste_bytes(&text, bracketed));
    }

    fn toggle_sidebar(&mut self, _: &ToggleSidebar, _: &mut Window, cx: &mut Context<Self>) {
        self.sidebar = !self.sidebar;
        cx.notify();
    }

    fn key_down(&mut self, ev: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let ks = &ev.keystroke;
        let m = &ks.modifiers;
        if m.platform {
            let digit = ks.key.parse::<usize>().ok().filter(|d| (1..=9).contains(d));
            if let Some(d) = digit.filter(|_| !(m.control || m.alt || m.shift)) {
                self.focus_window(d - 1, cx);
                cx.stop_propagation();
            }
            return;
        }
        let Some(t) = self.terminal() else { return };
        let app_cursor = t.with_term(|term| term.mode().contains(TermMode::APP_CURSOR));
        if let Some(bytes) = crate::input::key_bytes(ks, app_cursor) {
            let _ = t.write(&bytes);
            cx.stop_propagation();
        }
    }

    fn select(&mut self, session: &str, window: Option<usize>, cx: &mut Context<Self>) {
        self.session_id = Some(session.to_string());
        match window {
            Some(i) => self.focus_window(i, cx),
            None => cx.notify(),
        }
    }

    fn render_sidebar(&self, rows: &[SessionRow], cx: &mut Context<Self>) -> impl IntoElement {
        let t = &DARK;
        let mut list = div().flex().flex_col().gap(px(2.)).px(px(8.));
        for (i, row) in rows.iter().enumerate() {
            let selected = self.session_id.as_deref() == Some(row.id.as_str());
            let open = !self.collapsed.contains(&row.id);
            let (id, toggle_id) = (row.id.clone(), row.id.clone());
            let header = div()
                .id(SharedString::from(format!("session-{i}")))
                .flex()
                .items_center()
                .gap(px(6.))
                .h(px(28.))
                .px(px(6.))
                .rounded(px(6.))
                .cursor_pointer()
                .when(selected, |d| d.bg(rgb(t.hover)).text_color(rgb(0xffffff)))
                .when(!selected, |d| d.text_color(rgb(t.text)).hover(|d| d.bg(rgb(t.hover))))
                .child(
                    div()
                        .id(SharedString::from(format!("chevron-{i}")))
                        .w(px(12.))
                        .text_color(rgb(t.muted))
                        .child(if open { "▾" } else { "▸" })
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |ws, _, _, cx| {
                                if !ws.collapsed.remove(&toggle_id) {
                                    ws.collapsed.insert(toggle_id.clone());
                                }
                                cx.stop_propagation();
                                cx.notify();
                            }),
                        ),
                )
                .child(div().flex_1().overflow_hidden().text_ellipsis().child(row.title.clone()))
                .child(
                    div()
                        .text_xs()
                        .px(px(6.))
                        .rounded(px(8.))
                        .bg(rgb(t.border))
                        .text_color(rgb(t.muted))
                        .child(row.windows.len().to_string()),
                )
                .on_mouse_down(MouseButton::Left, cx.listener(move |ws, _, _, cx| ws.select(&id, None, cx)));
            list = list.child(header);
            if open {
                for (w, win) in row.windows.iter().enumerate() {
                    let active = w == row.active;
                    let sid = row.id.clone();
                    list = list.child(
                        div()
                            .id(SharedString::from(format!("window-{i}-{w}")))
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .h(px(24.))
                            .pl(px(26.))
                            .pr(px(6.))
                            .rounded(px(6.))
                            .text_sm()
                            .cursor_pointer()
                            .text_color(rgb(if active && selected { t.text } else { t.muted }))
                            .hover(|d| d.bg(rgb(t.hover)))
                            .child(div().size(px(6.)).rounded_full().bg(rgb(if active { t.accent } else { t.border })))
                            .child(div().flex_1().overflow_hidden().text_ellipsis().child(win.title.clone()))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |ws, _, _, cx| ws.select(&sid, Some(w), cx)),
                            ),
                    );
                }
            }
        }
        div()
            .flex()
            .flex_col()
            .w(px(SIDEBAR_WIDTH))
            .h_full()
            .flex_none()
            .bg(rgb(t.chrome))
            .border_r_1()
            .border_color(rgb(t.border))
            .child(
                div()
                    .mt(px(TAB_BAR_HEIGHT)) // the traffic lights' row
                    .h(px(28.))
                    .flex()
                    .items_center()
                    .justify_between()
                    .px(px(14.))
                    .text_xs()
                    .text_color(rgb(t.muted))
                    .child("SESSIONS")
                    .child(
                        div()
                            .id("new-session")
                            .px(px(6.))
                            .rounded(px(4.))
                            .cursor_pointer()
                            .hover(|d| d.bg(rgb(t.hover)).text_color(rgb(t.text)))
                            .child("+")
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|ws, _, window, cx| ws.new_session(&NewSession, window, cx)),
                            ),
                    ),
            )
            .child(div().flex_1().child(list))
            .child(
                div().p(px(14.)).text_xs().whitespace_nowrap().text_color(rgb(t.muted)).child("⌘N session  ·  ⌘T tab"),
            )
    }

    fn render_tabs(&self, row: Option<&SessionRow>, cx: &mut Context<Self>) -> impl IntoElement {
        let t = &DARK;
        let mut bar = div()
            .flex()
            .flex_none()
            .items_end()
            .h(px(TAB_BAR_HEIGHT))
            .bg(rgb(t.chrome))
            .border_b_1()
            .border_color(rgb(t.border))
            .px(px(6.))
            .when(!self.sidebar, |d| d.pl(px(78.))) // clear the traffic lights (after px, which sets left too)
            .gap(px(2.));
        if let Some(row) = row {
            for (i, w) in row.windows.iter().enumerate() {
                let active = i == row.active;
                let (sid, wid) = (row.id.clone(), w.id.clone());
                bar = bar.child(
                    div()
                        .id(SharedString::from(format!("tab-{i}")))
                        .group("tab")
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .h(px(30.))
                        .px(px(12.))
                        .min_w(px(120.))
                        .max_w(px(220.))
                        .rounded_t(px(7.))
                        .text_sm()
                        .cursor_pointer()
                        .when(active, |d| {
                            d.bg(rgb(t.bg)).text_color(rgb(0xffffff)).border_t_2().border_color(rgb(t.accent))
                        })
                        .when(!active, |d| {
                            d.text_color(rgb(t.muted)).hover(|d| d.bg(rgb(t.hover)).text_color(rgb(t.text)))
                        })
                        .child(div().flex_1().overflow_hidden().text_ellipsis().child(w.title.clone()))
                        .when(i < 9, |d| d.child(div().text_xs().text_color(rgb(t.muted)).child(format!("⌘{}", i + 1))))
                        .child(
                            div()
                                .id(SharedString::from(format!("close-{i}")))
                                .px(px(4.))
                                .rounded(px(4.))
                                .text_color(rgb(t.muted))
                                .invisible()
                                .group_hover("tab", |d| d.visible())
                                .hover(|d| d.bg(rgb(t.border)).text_color(rgb(t.text)))
                                .child("×")
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |ws, _, _, cx| {
                                        ws.close_window((sid.clone(), wid.clone()), cx);
                                        cx.stop_propagation();
                                    }),
                                ),
                        )
                        .on_mouse_down(MouseButton::Left, cx.listener(move |ws, _, _, cx| ws.focus_window(i, cx))),
                );
            }
        }
        bar.child(
            div()
                .id("new-tab")
                .flex()
                .items_center()
                .justify_center()
                .size(px(28.))
                .mb(px(1.))
                .rounded(px(6.))
                .cursor_pointer()
                .text_color(rgb(t.muted))
                .hover(|d| d.bg(rgb(t.hover)).text_color(rgb(t.text)))
                .child("+")
                .on_mouse_down(MouseButton::Left, cx.listener(|ws, _, window, cx| ws.new_tab(&NewTab, window, cx))),
        )
    }
}

impl Workspace {
    /// The active window's panes, placed by `panes::layout` in the pane area: the window less the
    /// sidebar and the tab bar (so the size is known now, not a frame late).
    fn render_panes(
        &self,
        w: &leon_core::Window,
        terms: Panes,
        window: &Window,
        stats: Rc<RefCell<Stats>>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let t = &DARK;
        let view = window.viewport_size();
        let sidebar = if self.sidebar { SIDEBAR_WIDTH } else { 0. };
        let size = ((f32::from(view.width) - sidebar).max(0.), (f32::from(view.height) - TAB_BAR_HEIGHT).max(0.));
        let active = window.is_window_active();
        let l =
            w.layout.as_ref().map(|tree| panes::layout(tree, w.zoomed.as_deref(), size, DIVIDER)).unwrap_or_default();
        let target = target(w).map(str::to_string);
        let several = l.panes.len() > 1;
        let mut out = div().flex_1().relative().bg(rgb(t.bg));
        for p in &l.panes {
            let Some((_, term)) = terms.iter().find(|(id, _)| *id == p.block_id) else { continue };
            let focused = target.as_deref() == Some(p.block_id.as_str());
            let id = p.block_id.clone();
            out = out.child(
                div()
                    .id(SharedString::from(format!("pane-{}", p.block_id)))
                    .debug_selector(|| format!("pane-{}", p.block_id))
                    .absolute()
                    .left(px(p.rect.x))
                    .top(px(p.rect.y))
                    .w(px(p.rect.w))
                    .h(px(p.rect.h))
                    .p(px(PADDING - 1.))
                    .border_1()
                    .border_color(if several && focused {
                        gpui::Hsla::from(rgb(t.accent)).opacity(0.55)
                    } else {
                        gpui::transparent_black()
                    })
                    .child(terminal_canvas(term.clone(), active && focused, stats.clone()))
                    .on_mouse_down(MouseButton::Left, cx.listener(move |ws, _, _, cx| ws.focus_block(&id, cx))),
            );
        }
        for (i, d) in l.dividers.iter().enumerate() {
            let r = d.rect;
            let across = d.direction == Direction::Horizontal;
            let (x, y, w, h) = if across {
                (r.x - DIVIDER_GRAB, r.y, r.w + 2. * DIVIDER_GRAB, r.h)
            } else {
                (r.x, r.y - DIVIDER_GRAB, r.w, r.h + 2. * DIVIDER_GRAB)
            };
            let divider = d.clone();
            let line = if across { div().w(px(DIVIDER)).h_full() } else { div().h(px(DIVIDER)).w_full() };
            out = out.child(
                div()
                    .id(SharedString::from(format!("divider-{i}")))
                    .debug_selector(|| format!("divider-{i}"))
                    .absolute()
                    .left(px(x))
                    .top(px(y))
                    .w(px(w))
                    .h(px(h))
                    .flex()
                    .justify_center()
                    .items_center()
                    .occlude()
                    .cursor(if across { CursorStyle::ResizeLeftRight } else { CursorStyle::ResizeUpDown })
                    .child(line.bg(rgb(t.border)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |ws, ev: &gpui::MouseDownEvent, _, cx| {
                            ws.start_drag(divider.clone(), ev.position, cx)
                        }),
                    ),
            );
        }
        out
    }
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let stats = self.stats.clone();
        if stats.borrow().enabled {
            stats.borrow_mut().render_start = Some(Instant::now());
        }
        let rows = self.rows();
        self.settle(&rows);
        let current = rows.iter().find(|r| Some(&r.id) == self.session_id.as_ref());
        let body = match self.shown() {
            Some((w, terms)) => self.render_panes(&w, terms, window, stats, cx).into_any_element(),
            None => div()
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(6.))
                .bg(rgb(DARK.bg))
                .child(div().text_color(rgb(DARK.text)).child("No sessions"))
                .child(div().text_sm().text_color(rgb(DARK.muted)).child("Press ⌘N to start one"))
                .into_any_element(),
        };
        div()
            .key_context("Workspace")
            .track_focus(&self.focus)
            .on_action(cx.listener(Self::new_tab))
            .on_action(cx.listener(Self::close_pane))
            .on_action(cx.listener(Self::split_right))
            .on_action(cx.listener(Self::split_down))
            .on_action(cx.listener(Self::focus_left))
            .on_action(cx.listener(Self::focus_right))
            .on_action(cx.listener(Self::focus_up))
            .on_action(cx.listener(Self::focus_down))
            .on_action(cx.listener(Self::toggle_zoom))
            .child(self.drag_listener(cx))
            .on_action(cx.listener(Self::new_session))
            .on_action(cx.listener(Self::next_tab))
            .on_action(cx.listener(Self::previous_tab))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::toggle_sidebar))
            .on_key_down(cx.listener(Self::key_down))
            .size_full()
            .flex()
            .bg(rgb(DARK.bg))
            .font_family(".SystemUIFont")
            .text_color(rgb(DARK.text))
            .when(self.sidebar, |d| d.child(self.render_sidebar(&rows, cx)))
            .child(div().flex_1().flex().flex_col().min_w(px(0.)).child(self.render_tabs(current, cx)).child(body))
    }
}

/// The argv0 a tab shows: `/bin/zsh` → `zsh`, a login shell's `-zsh` → `zsh`.
fn program_name(command: &[String]) -> String {
    let argv0 = command.first().map_or("", String::as_str);
    argv0.rsplit('/').next().unwrap_or(argv0).trim_start_matches('-').to_string()
}

fn terminal_font(bold: bool, italic: bool) -> Font {
    Font {
        weight: if bold { FontWeight::BOLD } else { FontWeight::NORMAL },
        style: if italic { FontStyle::Italic } else { FontStyle::Normal },
        ..font(FONT_FAMILY)
    }
}

/// Paints one terminal. Prepaint fits the grid to the bounds (resizing the PTY when rows or
/// columns change) and builds the render model under the terminal's lock; paint draws it.
fn terminal_canvas(term: Arc<Terminal>, active: bool, stats: Rc<RefCell<Stats>>) -> impl IntoElement {
    let font_size = px(FONT_SIZE);
    canvas(
        move |bounds, window, _| {
            let ts = window.text_system();
            let font_id = ts.resolve_font(&terminal_font(false, false));
            let cell_w =
                ts.advance(font_id, font_size, 'm').map(|s| s.width).ok().filter(|w| *w > px(0.)).unwrap_or(px(8.));
            let line_h = (font_size * LINE_HEIGHT).round();
            let fit = |len: Pixels, unit: Pixels, least: u16| ((len / unit).floor() as u16).max(least);
            // At least what `Terminal::resize` clamps to, or every frame would resize again.
            let (rows, cols) = (fit(bounds.size.height, line_h, 1), fit(bounds.size.width, cell_w, 2));
            if term.size() != (rows, cols) {
                let _ = term.resize(rows, cols);
            }
            let frame = term.with_term(|t| render::frame(t, &DARK));
            (frame, cell_w, line_h)
        },
        move |bounds, (frame, cell_w, line_h), window, cx| {
            paint_frame(&frame, &DARK, bounds, cell_w, line_h, active, window, cx);
            stats.borrow_mut().painted();
        },
    )
    .size_full()
}

#[allow(clippy::too_many_arguments)]
fn paint_frame(
    f: &Frame,
    t: &Theme,
    bounds: Bounds<Pixels>,
    cell_w: Pixels,
    line_h: Pixels,
    active: bool,
    window: &mut Window,
    cx: &mut App,
) {
    let at =
        |line: usize, col: usize| point(bounds.origin.x + cell_w * col as f32, bounds.origin.y + line_h * line as f32);
    for r in &f.backgrounds {
        window.paint_quad(fill(Bounds::new(at(r.line, r.col), size(cell_w * r.len as f32, line_h)), rgb(r.color)));
    }
    let mark = f.cursor.as_ref().map(|c| {
        let w = cell_w * if c.wide { 2. } else { 1. };
        let origin = at(c.line, c.col);
        match c.shape {
            CursorShape::Beam => Mark::Bar(Bounds::new(origin, size(px(2.), line_h))),
            CursorShape::Underline => {
                Mark::Bar(Bounds::new(point(origin.x, origin.y + line_h - px(2.)), size(w, px(2.))))
            }
            CursorShape::Block if active => Mark::Block(Bounds::new(origin, size(w, line_h))),
            _ => Mark::Hollow(Bounds::new(origin, size(w, line_h))), // also any block in an inactive window
        }
    });
    let mut paint_text =
        |text: &str, line: usize, col: usize, color: u32, s: Option<&render::Span>, window: &mut Window| {
            let (bold, italic, underline, strike) =
                s.map_or((false, false, false, false), |s| (s.bold, s.italic, s.underline, s.strikeout));
            let run = TextRun {
                len: text.len(),
                font: terminal_font(bold, italic),
                color: rgb(color).into(),
                background_color: None,
                underline: underline.then(|| UnderlineStyle { thickness: px(1.), color: None, wavy: false }),
                strikethrough: strike.then(|| StrikethroughStyle { thickness: px(1.), color: None }),
            };
            // Pin glyph i to column i, so a fallback font's advance can't push text off the grid.
            let grid = s.filter(|s| s.cells == s.text.chars().count()).map(|_| cell_w);
            let shaped =
                window.text_system().shape_line(SharedString::from(text.to_string()), px(FONT_SIZE), &[run], grid);
            let _ = shaped.paint(at(line, col), line_h, window, cx);
        };
    // A solid block cursor sits under the text; its cell's text is redrawn in the background color.
    if let Some(Mark::Block(b)) = mark {
        window.paint_quad(fill(b, rgb(t.cursor)));
    }
    for (line, spans) in f.rows.iter().enumerate() {
        for s in spans {
            paint_text(&s.text, line, s.col, s.fg, Some(s), window);
        }
    }
    match (mark, &f.cursor) {
        (Some(Mark::Block(_)), Some(c)) if !c.text.trim().is_empty() => {
            paint_text(&c.text, c.line, c.col, t.bg, None, window)
        }
        (Some(Mark::Bar(b)), _) => window.paint_quad(fill(b, rgb(t.cursor))),
        (Some(Mark::Hollow(b)), _) => window.paint_quad(outline(b, rgb(t.cursor), gpui::BorderStyle::Solid)),
        _ => {}
    }
}

/// How the cursor is drawn.
enum Mark {
    /// Filled, under the text.
    Block(Bounds<Pixels>),
    /// A beam or underline bar, over the text.
    Bar(Bounds<Pixels>),
    /// An outline: a hollow-block cursor, or the cursor of an inactive window.
    Hollow(Bounds<Pixels>),
}
