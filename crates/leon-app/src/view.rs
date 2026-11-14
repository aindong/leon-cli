//! The window: a sidebar of sessions, a tab bar of the selected session's windows, and the active
//! window's split layout, each pane a terminal painted from the in-process grid. A poll on the
//! GPUI executor redraws only when a visible terminal or the session model changed.

use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use alacritty_terminal::term::TermMode;
use alacritty_terminal::vte::ansi::CursorShape;
use gpui::{
    actions, canvas, div, fill, font, outline, point, prelude::*, px, rgb, size, App, Application, Bounds, Context,
    CursorStyle, FocusHandle, Font, FontStyle, FontWeight, KeyBinding, KeyDownEvent, Menu, MenuItem, MouseButton,
    MouseMoveEvent, MouseUpEvent, Pixels, SharedString, StrikethroughStyle, Subscription, Task, TextRun,
    TitlebarOptions, UnderlineStyle, Window, WindowBounds, WindowOptions,
};
use leon_core::layout::Direction;
use leon_core::terminal::Terminal;
use leon_lua::keys::Chord;
use serde_json::{json, Value};

use crate::embedded::Embedded;
use crate::keymap::{self, Bound, Keymap, Pressed};
use crate::panes::{self, Divider};
use crate::render::{self, Frame};
use crate::status::{self, Indicator};
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
/// The terminal font size in points: the default, and the range cmd+= / cmd+- move in.
const FONT_SIZE: f32 = 13.;
const FONT_SIZES: (f32, f32) = (8., 32.);
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
/// The spinner's frame time: at most 8 redraws a second for it.
const SPIN: Duration = Duration::from_millis(125);
/// How often the app asks the server for its keymap (a reload elsewhere shows up this late).
const KEYMAP_POLL: Duration = Duration::from_secs(1);

/// The block input goes to: the one on screen when a block is zoomed, else the focused one. The
/// model keeps zoom and focus apart (a CLI can zoom any block), so this is what the user sees.
fn target(w: &leon_core::Window) -> Option<&str> {
    w.zoomed.as_deref().or(w.focus.as_deref())
}

/// GPUI key bindings: only Quit. Every other shortcut goes through `key_down` and the keymap
/// (`keymap::DEFAULTS` under the config's bindings), since GPUI matches its own bindings before
/// any key listener and a config could never override them.
pub fn init(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
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
    status: Option<Indicator>,
}

struct WindowRow {
    id: String,
    title: String,
    status: Option<Indicator>,
}

/// The palette while it is open.
struct Palette {
    query: String,
    selected: usize,
}

/// The last `client.keymap` reply, handed over from the runtime thread that fetched it.
#[derive(Default)]
struct KeymapInbox {
    asking: bool,
    reply: Option<Value>,
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
    keymap: Keymap,
    inbox: Arc<Mutex<KeymapInbox>>,
    keymap_asked: Option<Instant>,
    /// The chords of a sequence typed so far (`ctrl+b`).
    pending: Vec<Chord>,
    palette: Option<Palette>,
    font_size: f32,
    /// Where the spinner's clock starts.
    epoch: Instant,
    /// The pane last seen with the focus, and whether the window was active: a change is the user
    /// arriving at a pane, which clears its done/error.
    seen_focus: Option<(String, bool)>,
    _poll: Task<()>,
    _activation: Subscription,
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
                ws.refresh_keymap(cx);
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
        let activation = cx.observe_window_activation(window, |_, _, cx| cx.notify());
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
            keymap: Keymap::new(&json!({})),
            inbox: Arc::default(),
            keymap_asked: None,
            pending: Vec::new(),
            palette: None,
            font_size: FONT_SIZE,
            epoch: Instant::now(),
            seen_focus: None,
            _poll: poll,
            _activation: activation,
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
            .map(|(i, s)| {
                let shows = |b: &leon_core::Block| status::indicator(&b.terminal.program_status());
                let windows: Vec<WindowRow> = s
                    .windows
                    .iter()
                    .map(|w| WindowRow {
                        id: w.id.clone(),
                        title: w.label.clone().unwrap_or_else(|| {
                            let shown = target(w).and_then(|id| s.blocks.iter().find(|b| b.id == id));
                            shown.map_or("empty".into(), |b| program_name(&b.terminal.command))
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

    /// (each session's indicator, each tab's of the selected session), as the sidebar and tab bar show.
    pub fn indicators(&self) -> (Vec<Option<Indicator>>, Vec<Option<Indicator>>) {
        let rows = self.rows();
        let tabs = rows.iter().find(|r| Some(&r.id) == self.session_id.as_ref());
        let tabs = tabs.map(|r| r.windows.iter().map(|w| w.status).collect()).unwrap_or_default();
        (rows.iter().map(|r| r.status).collect(), tabs)
    }

    /// The status caption in a pane's corner.
    pub fn pane_caption(&self, block_id: &str) -> Option<String> {
        let t = self.app.ctx().state().block(block_id).ok()?.1.terminal.clone();
        status::caption(&t.program_status())
    }

    /// The spinner frame, when a working indicator is on screen (a sidebar row or a tab).
    fn spinning(&self, rows: &[SessionRow]) -> Option<u64> {
        let working = |i: &Option<Indicator>| matches!(i, Some(Indicator::Working { .. }));
        let selected = rows.iter().filter(|r| Some(&r.id) == self.session_id.as_ref());
        let on_screen = (self.sidebar && rows.iter().any(|r| working(&r.status)))
            || selected.flat_map(|r| &r.windows).any(|w| working(&w.status));
        on_screen.then(|| (self.epoch.elapsed().as_millis() / SPIN.as_millis()) as u64)
    }

    pub fn font_size(&self) -> f32 {
        self.font_size
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
        // status shows for every tab and session, shown or not
        let rows = self.rows();
        for r in &rows {
            r.status.hash(&mut h);
            r.windows.iter().for_each(|w| w.status.hash(&mut h));
        }
        self.spinning(&rows).hash(&mut h);
        (self.sidebar, self.font_size.to_bits(), self.pending.len(), self.palette.is_some()).hash(&mut h);
        self.session_id.hash(&mut h);
        for (id, t) in self.shown().map(|(_, terms)| terms).unwrap_or_default() {
            (id, Arc::as_ptr(&t) as usize, t.generation()).hash(&mut h);
            status::caption(&t.program_status()).hash(&mut h);
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
}

#[path = "view_chrome.rs"]
mod chrome;
#[path = "view_handlers.rs"]
mod handlers;
#[path = "view_keys.rs"]
mod keys;
#[path = "view_overlay.rs"]
mod overlay;
#[path = "view_panes.rs"]
mod panes_view;
use panes_view::*;
