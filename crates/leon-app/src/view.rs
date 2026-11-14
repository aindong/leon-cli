//! The window: a sidebar of sessions, a tab bar of the selected session's windows, and the active
//! window's terminal painted from the in-process grid. A poll on the GPUI executor redraws only
//! when a terminal or the session model changed.

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
    FocusHandle, Font, FontStyle, FontWeight, KeyBinding, KeyDownEvent, Menu, MenuItem, MouseButton, Pixels,
    SharedString, StrikethroughStyle, Task, TextRun, TitlebarOptions, UnderlineStyle, Window, WindowBounds,
    WindowOptions,
};
use leon_core::model::{Block, Session};
use leon_core::terminal::Terminal;
use serde_json::json;

use crate::embedded::Embedded;
use crate::render::{self, Frame};
use crate::theme::{Theme, DARK};

actions!(leon, [NewTab, CloseTab, NewSession, NextTab, PreviousTab, Paste, ToggleSidebar, Quit]);

const FONT_FAMILY: &str = "Menlo";
const FONT_SIZE: f32 = 13.;
const LINE_HEIGHT: f32 = 1.3;
const SIDEBAR_WIDTH: f32 = 220.;
const TAB_BAR_HEIGHT: f32 = 38.;
const PADDING: f32 = 10.;
/// How often the poll looks for changes (about 120 Hz).
const POLL: Duration = Duration::from_millis(8);

/// The block a window shows. One per window until M3's splits; then the window's focused block.
pub fn window_block<'a>(s: &'a Session, window_id: &str) -> Option<&'a Block> {
    s.blocks.iter().find(|b| b.window_id == window_id)
}

/// Key bindings. Cmd+1..9 are handled in `key_down`, so one rule covers them all.
pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-t", NewTab, None),
        KeyBinding::new("cmd-w", CloseTab, None),
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
                    MenuItem::action("Close Tab", CloseTab),
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
        let on_quit = server.clone();
        cx.on_action(move |_: &Quit, cx| {
            on_quit.stop();
            cx.quit();
        });
        let on_close = server.clone();
        cx.on_window_closed(move |cx| {
            on_close.stop();
            cx.quit();
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

pub struct Workspace {
    app: Rc<Embedded>,
    session_id: Option<String>,
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
            session_id,
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

    /// The terminal of the selected session's active window.
    pub fn terminal(&self) -> Option<Arc<Terminal>> {
        let st = self.app.ctx().state();
        let s = st.session(self.session_id.as_deref()?).ok()?;
        Some(window_block(s, &s.windows.get(s.active)?.id)?.terminal.clone())
    }

    /// Keep the selection on a live session: a closed one hands over to its neighbour.
    fn settle(&mut self, rows: &[SessionRow], before: Option<usize>) {
        if self.session_id.as_ref().is_some_and(|id| rows.iter().any(|r| &r.id == id)) {
            return;
        }
        let i = before.unwrap_or(0).min(rows.len().saturating_sub(1));
        self.session_id = rows.get(i).map(|r| r.id.clone());
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
                            window_block(s, &w.id).map_or("empty".into(), |b| program_name(&b.terminal.command))
                        }),
                    })
                    .collect(),
                active: s.active,
            })
            .collect()
    }

    /// Changes worth a redraw: the model's shape and the shown terminal's content.
    fn signature(&self) -> u64 {
        let mut h = DefaultHasher::new();
        {
            let st = self.app.ctx().state();
            for s in &st.sessions {
                (&s.id, &s.label, s.active).hash(&mut h);
                for w in &s.windows {
                    (&w.id, &w.label).hash(&mut h);
                }
            }
        }
        self.session_id.hash(&mut h);
        self.terminal().map(|t| (Arc::as_ptr(&t) as usize, t.generation())).hash(&mut h);
        h.finish()
    }

    fn session_index(&self) -> Option<usize> {
        let id = self.session_id.as_deref()?;
        self.app.ctx().state().sessions.iter().position(|s| s.id == id)
    }

    fn focus_window(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(sid) = self.session_id.clone() else { return };
        let wid = self.app.ctx().state().session(&sid).ok().and_then(|s| s.windows.get(index).map(|w| w.id.clone()));
        if let Some(wid) = wid {
            let _ = self.app.call("session.focus_window", json!({ "session_id": sid, "window_id": wid }));
            cx.notify();
        }
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

    fn close_tab(&mut self, _: &CloseTab, _: &mut Window, cx: &mut Context<Self>) {
        let Some(sid) = self.session_id.clone() else { return };
        let before = self.session_index();
        let wid = self.app.ctx().state().session(&sid).ok().and_then(|s| s.windows.get(s.active).map(|w| w.id.clone()));
        if let Some(wid) = wid {
            let _ = self.app.call("session.close_window", json!({ "session_id": sid, "window_id": wid }));
        }
        let rows = self.rows();
        self.settle(&rows, before);
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
                    .h(px(TAB_BAR_HEIGHT + 6.))
                    .flex()
                    .items_end()
                    .justify_between()
                    .px(px(14.))
                    .pb(px(6.))
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
            .child(div().p(px(14.)).text_xs().text_color(rgb(t.muted)).child("⌘N session · ⌘T tab · ⌘B sidebar"))
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
            .when(!self.sidebar, |d| d.pl(px(78.))) // clear the traffic lights
            .px(px(6.))
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
                                        let p = json!({ "session_id": sid, "window_id": wid });
                                        let before = ws.session_index();
                                        let _ = ws.app.call("session.close_window", p);
                                        let rows = ws.rows();
                                        ws.settle(&rows, before);
                                        cx.stop_propagation();
                                        cx.notify();
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

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let stats = self.stats.clone();
        if stats.borrow().enabled {
            stats.borrow_mut().render_start = Some(Instant::now());
        }
        let rows = self.rows();
        self.settle(&rows, None);
        let current = rows.iter().find(|r| Some(&r.id) == self.session_id.as_ref());
        let terminal = self.terminal();
        let active = window.is_window_active();
        let body = match terminal {
            Some(term) => div().flex_1().bg(rgb(DARK.bg)).p(px(PADDING)).child(terminal_canvas(term, active, stats)),
            None => div()
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(6.))
                .bg(rgb(DARK.bg))
                .child(div().text_color(rgb(DARK.text)).child("No sessions"))
                .child(div().text_sm().text_color(rgb(DARK.muted)).child("Press ⌘N to start one")),
        };
        div()
            .key_context("Workspace")
            .track_focus(&self.focus)
            .on_action(cx.listener(Self::new_tab))
            .on_action(cx.listener(Self::close_tab))
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
            let fit = |len: Pixels, unit: Pixels| ((len / unit).floor() as u16).max(1);
            let (rows, cols) = (fit(bounds.size.height, line_h), fit(bounds.size.width, cell_w));
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
    let cursor_bounds = f.cursor.as_ref().map(|c| {
        let w = cell_w * if c.wide { 2. } else { 1. };
        let origin = at(c.line, c.col);
        match (c.shape, active) {
            (CursorShape::Beam, _) => (Bounds::new(origin, size(px(2.), line_h)), true),
            (CursorShape::Underline, _) => {
                (Bounds::new(point(origin.x, origin.y + line_h - px(2.)), size(w, px(2.))), true)
            }
            (_, true) => (Bounds::new(origin, size(w, line_h)), false),
            (_, false) => (Bounds::new(origin, size(w, line_h)), true),
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
            let shaped =
                window.text_system().shape_line(SharedString::from(text.to_string()), px(FONT_SIZE), &[run], None);
            let _ = shaped.paint(at(line, col), line_h, window, cx);
        };
    // A solid block cursor sits under the text; its cell's text is redrawn in the background color.
    if let Some((b, false)) = cursor_bounds {
        window.paint_quad(fill(b, rgb(t.cursor)));
    }
    for (line, spans) in f.rows.iter().enumerate() {
        for s in spans {
            paint_text(&s.text, line, s.col, s.fg, Some(s), window);
        }
    }
    match (cursor_bounds, &f.cursor) {
        (Some((_, false)), Some(c)) if c.text.trim() != "" => paint_text(&c.text, c.line, c.col, t.bg, None, window),
        (Some((b, true)), Some(c)) if matches!(c.shape, CursorShape::Beam | CursorShape::Underline) => {
            window.paint_quad(fill(b, rgb(t.cursor)))
        }
        (Some((b, true)), _) => window.paint_quad(outline(b, rgb(t.cursor), gpui::BorderStyle::Solid)),
        _ => {}
    }
}
