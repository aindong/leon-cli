//! The window: a sidebar of sessions (with their tabs as rows, or in top tab style only the
//! sessions, the tabs then in the titlebar), and the active window's split layout as tiles, each a
//! pane header over a terminal painted from the screen the server pushes. A task
//! on the GPUI executor redraws when the model or a screen changed, woken by the reader thread
//! (or every `POLL`, for the spinner and the keymap).

use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use alacritty_terminal::vte::ansi::CursorShape;
use futures::StreamExt;
use gpui::{
    actions, canvas, div, fill, font, outline, point, prelude::*, px, rgb, size, App, Application, Bounds, Context,
    CursorStyle, FocusHandle, Font, FontStyle, FontWeight, KeyBinding, KeyDownEvent, Menu, MenuItem, MouseButton,
    MouseMoveEvent, MouseUpEvent, Pixels, PromptButton, PromptLevel, SharedString, StrikethroughStyle, Subscription,
    Task, TextRun, TitlebarOptions, UnderlineStyle, Window, WindowBounds, WindowOptions,
};
use leon_core::layout::Direction;
use leon_lua::keys::Chord;
use serde_json::{json, Value};

use crate::chrome::{self, Places, Settings, SidebarList};
use crate::floats;
use crate::hosts::{HostState, Hosts};
use crate::icons::{Icon, ROW_ICON, TOOLBAR_ICON, TOOLBAR_TARGET};
use crate::keymap::{self, Bound, Keymap, Pressed};
use crate::look::Look;
use crate::panes::{self, Divider, GAP};
use crate::remote::Remote;
use crate::render::{self, Frame};
use crate::status::{self, Indicator};
use crate::switcher;
use crate::theme::{Theme, SECONDARY_TEXT, UI_TEXT};

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
        About,
        Quit,
        QuitAndEndAll,
        SplitRight,
        SplitDown,
        FocusLeft,
        FocusRight,
        FocusUp,
        FocusDown,
        ToggleZoom,
        Copy,
        SelectAll,
        Find,
        OpenSettings
    ]
);

/// The range cmd+= / cmd+- move the terminal font size in (points); init.lua's `font.size` is
/// the size cmd+0 returns to.
const FONT_SIZES: (f32, f32) = (leon_lua::settings::FONT_SIZES.0 as f32, leon_lua::settings::FONT_SIZES.1 as f32);
const LINE_HEIGHT: f32 = 1.3;
/// The titlebar row: the traffic lights, the toolbar, and in top style the tabs.
const TITLEBAR: f32 = 38.;
/// Room for the traffic lights at the left of the titlebar row.
const TRAFFIC_LIGHTS: f32 = 78.;
/// How often the poll looks for changes (about 120 Hz).
const POLL: Duration = Duration::from_millis(8);
/// The spinner's frame time: at most 8 redraws a second for it.
const SPIN: Duration = Duration::from_millis(125);
/// The largest paste sent: one request line is at most 1 MiB (`serve::MAX_LINE`), and JSON can
/// double a text's size.
const MAX_PASTE: usize = 400 << 10;
/// How often the app asks the server for its keymap (a reload elsewhere shows up this late).
const KEYMAP_POLL: Duration = Duration::from_secs(1);

/// The block input goes to.
use crate::floats::target;

/// What one render needs from the model, copied out under the state lock.
struct SessionRow {
    /// Which machine it is on (`Hosts` index; 0 is This Mac).
    host: usize,
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
    /// Asking for text instead of listing actions (Add Host…), with the last refusal.
    ask: Option<(hosts_view::Ask, Option<String>)>,
    /// The session switcher (⌘K): places to go instead of actions.
    switch: bool,
    /// Why the last ⌘↩ couldn't split (a row of another machine).
    note: Option<String>,
}

/// The last `client.keymap` reply, handed over from the runtime thread that fetched it.
#[derive(Default)]
struct KeymapInbox {
    asking: bool,
    reply: Option<Value>,
    /// The `server.settings` reply asked for when the keymap's generation moved to the one given.
    settings: Option<(u64, Value)>,
}

/// The blocks on screen, in leaf order.
type Panes = Vec<String>;

/// Something being dragged.
enum Drag {
    /// A divider: the split it sets, where the drag started, and the ratio last sent, so a move
    /// that changes nothing sends nothing.
    Divider { divider: Divider, from: Pixels, last: Option<f64> },
    /// The sidebar's edge: where the drag started and the width then.
    Sidebar { from: Pixels, width: f32 },
    /// A float's header or edge, or a drawer's divider (view_floats.rs).
    Overlay { block: String, grab: floats::Grab, from: gpui::Point<Pixels>, last: Option<floats::Placement> },
}

thread_local! {
    /// The debug selectors the last frame drew. Only test builds name elements, and GPUI keeps a
    /// selector's bounds after its element is gone, so tests ask this what is on screen.
    static DRAWN: RefCell<HashSet<String>> = RefCell::default();
}

/// `name`, noted as drawn this frame: what each `debug_selector` closure returns.
fn sel(name: String) -> String {
    DRAWN.with(|d| d.borrow_mut().insert(name.clone()));
    name
}

/// The debug selectors of the elements the last frame drew (empty outside test builds).
pub fn drawn() -> HashSet<String> {
    DRAWN.with(|d| d.borrow().clone())
}

// Truncating text cells use `line_clamp(1)`, not `whitespace_nowrap`: GPUI 0.2.2 caches a no-wrap
// text's size from its first layout (before its width is known) and never adds the ellipsis.

/// A GPUI rectangle as a settings frame.
fn frame(b: Bounds<Pixels>) -> chrome::Frame {
    chrome::Frame { x: b.origin.x.into(), y: b.origin.y.into(), w: b.size.width.into(), h: b.size.height.into() }
}

/// Where the app keeps its settings: `$LEON_STATE_DIR/app.json` (next to the server's sessions).
pub fn settings_path() -> Option<PathBuf> {
    leon_core::paths::state_dir().map(|d| d.join("app.json"))
}

pub struct Workspace {
    /// The selected session's machine's remote: what every action acts on.
    app: Rc<Remote>,
    /// This Mac and the hosts; `host` is the one `app` belongs to.
    hosts: Hosts,
    host: usize,
    /// A session to select once it shows in its host's model (New Session over a link).
    select_later: Arc<Mutex<Option<(usize, String)>>>,
    drag: Option<Drag>,
    session_id: Option<String>,
    /// Where the selected session sat in the list when last seen.
    selected_at: usize,
    sidebar: bool,
    settings: Settings,
    /// Where `settings` are saved; None keeps them in memory (tests).
    settings_path: Option<PathBuf>,
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
    /// The window after a repeatable binding ran (tmux `bind -r`).
    repeat: keymap::Repeat,
    palette: Option<Palette>,
    /// Window ids of the tabs shown, most recent first (the switcher's order).
    recent: Vec<String>,
    /// The Activity list shows in the sidebar's place.
    activity: bool,
    attention: crate::attention::Tracker,
    notifier: crate::attention::Notifier,
    /// The Dock badge as last set.
    dock: Option<String>,
    /// Post notifications and set the Dock badge (the real app; never tests).
    platform: bool,
    /// The window is the key window: the user is looking at leon.
    window_active: bool,
    font_size: f32,
    /// init.lua's settings as drawn (`look.rs`); the theme for the OS appearance now (`dark`),
    /// and the terminal font. `refit` fits every pane again at the next render (a new font).
    look: Look,
    dark: bool,
    theme: Theme,
    font_family: SharedString,
    refit: bool,
    /// The config the local server loaded, from its keymap: where the theme files and the
    /// Settings window's init.lua are.
    config_path: Option<PathBuf>,
    /// The Settings window, while it is open.
    settings_window: Option<gpui::WindowHandle<crate::settings_window::SettingsWindow>>,
    /// Where the spinner's clock starts.
    epoch: Instant,
    /// The pane last seen with the focus, and whether the window was active: a change is the user
    /// arriving at a pane, which clears its done/error.
    seen_focus: Option<(String, bool)>,
    /// Scrollback, selection, find and links per pane.
    term: scroll::TermUi,
    floats: floats_view::FloatUi,
    _poll: Task<()>,
    _activation: Subscription,
    _bounds: Subscription,
    _quit: Subscription,
    _release: Subscription,
    _appearance: Subscription,
}

impl Workspace {
    /// A workspace whose settings live in memory only.
    pub fn new(app: Rc<Remote>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::with_settings(app, None, window, cx)
    }

    /// The app's own workspace: settings from `settings_path()`, saved there on each change, and
    /// the hosts list's machines.
    pub fn for_app(app: Rc<Remote>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let hosts = leon_hosts::hosts::load().unwrap_or_else(|e| {
            eprintln!("leon: can't read the hosts list: {e}");
            Vec::new()
        });
        let mut ws = Self::with_hosts(app, settings_path(), hosts, window, cx);
        ws.platform = true;
        crate::notify::prepare();
        ws
    }

    /// A workspace with the settings in `path` (the defaults when it is None or unreadable).
    pub fn with_settings(app: Rc<Remote>, path: Option<PathBuf>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::with_hosts(app, path, Vec::new(), window, cx)
    }

    /// A workspace on This Mac's `app` that also shows `hosts`, connecting to each.
    pub fn with_hosts(
        app: Rc<Remote>,
        path: Option<PathBuf>,
        hosts: Vec<leon_hosts::Host>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        if app.model().sessions.is_empty() {
            let _ = app.act("session.create", json!({}));
        }
        let focus = cx.focus_handle();
        window.focus(&focus);
        // the reader thread wakes the poll at once; one pending wake is enough (one sender, so the
        // channel's bound holds: each clone would bring a slot of its own)
        let (wake, mut woken) = futures::channel::mpsc::channel::<()>(0);
        let wake = Mutex::new(wake);
        let wake: Arc<dyn Fn() + Send + Sync> =
            Arc::new(move || drop(wake.lock().unwrap_or_else(|e| e.into_inner()).try_send(())));
        let woke = wake.clone();
        app.on_change(move || woke());
        crate::notify::on_click(wake.clone());
        let mut term = scroll::TermUi::default();
        term.wake = Some(wake.clone());
        let hosts = Hosts::new(app.clone(), hosts, wake);
        let poll = cx.spawn(async move |this, cx| loop {
            let timer = cx.background_executor().timer(POLL);
            futures::future::select(timer, woken.next()).await;
            let alive = this.update(cx, |ws, cx| {
                if !ws.hosts.local().alive() {
                    cx.quit(); // the server stopped (`leon server stop`, or the menu's end-all)
                    return;
                }
                ws.poll_hosts();
                let replied = ws.poll_terms();
                if let Some(text) = ws.term.clip.take() {
                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
                }
                ws.poll_attention(cx);
                ws.stats.borrow_mut().report();
                ws.refresh_keymap(cx);
                let now = ws.signature();
                if now != ws.seen || ws.sliding() || replied {
                    ws.seen = now;
                    cx.notify();
                }
            });
            if alive.is_err() {
                return;
            }
        });
        let stats = Stats::new();
        let session_id = app.model().sessions.first().map(|s| s.id.clone());
        let activation = cx.observe_window_activation(window, |ws, window, cx| {
            ws.window_active = window.is_window_active();
            cx.notify()
        });
        // the window's frame and display are kept with the settings, and saved when the app quits
        // or the window closes (closing drops the workspace before the quit it asks for)
        let bounds = cx.observe_window_bounds(window, |ws, window, cx| {
            ws.settings.window = Some(frame(window.window_bounds().get_bounds()));
            ws.settings.display = window.display(cx).map(|d| u32::from(d.id()));
        });
        let quit = cx.on_app_quit(|ws, _| {
            ws.save_settings();
            async {}
        });
        let release = cx.on_release(|ws, _| ws.save_settings());
        let appearance = cx.observe_window_appearance(window, |ws, window, cx| {
            ws.set_appearance(look_view::is_dark(window.appearance()), cx)
        });
        let (look, dark) = (Look::default(), look_view::is_dark(window.appearance()));
        Workspace {
            app,
            hosts,
            host: 0,
            select_later: Arc::default(),
            drag: None,
            session_id,
            selected_at: 0,
            sidebar: true,
            settings: path.as_deref().map(Settings::load).unwrap_or_default(),
            settings_path: path,
            collapsed: HashSet::new(),
            focus,
            stats: Rc::new(RefCell::new(stats)),
            seen: 0,
            keymap: Keymap::app(&json!({})),
            inbox: Arc::default(),
            keymap_asked: None,
            pending: Vec::new(),
            repeat: keymap::Repeat::default(),
            palette: None,
            recent: Vec::new(),
            activity: false,
            attention: Default::default(),
            notifier: Default::default(),
            dock: None,
            platform: false,
            window_active: window.is_window_active(),
            font_size: look.font_size,
            theme: look.theme(dark),
            font_family: look.font_family.clone().into(),
            look,
            dark,
            refit: false,
            config_path: None,
            settings_window: None,
            epoch: Instant::now(),
            seen_focus: None,
            term,
            floats: Default::default(),
            _poll: poll,
            _activation: activation,
            _bounds: bounds,
            _quit: quit,
            _release: release,
            _appearance: appearance,
        }
    }

    /// The selected session, if any.
    pub fn session_id(&self) -> Option<String> {
        self.session_id.clone()
    }

    pub fn sidebar_visible(&self) -> bool {
        self.sidebar
    }

    /// The block input goes to (`target`) in the selected session's active window.
    pub fn target_block(&self) -> Option<String> {
        self.target().map(|(_, id, _)| id)
    }

    /// The selected session's active window, as the app last heard it from the server.
    pub fn active_window(&self) -> Option<leon_core::Window> {
        let m = self.app.model();
        let s = m.session(self.session_id.as_deref()?)?;
        s.windows.get(s.active).cloned()
    }

    /// The selected session's windows in tab order, and which is active.
    pub fn windows(&self) -> Option<(Vec<leon_core::Window>, usize)> {
        let m = self.app.model();
        let s = m.session(self.session_id.as_deref()?)?;
        Some((s.windows.clone(), s.active))
    }

    /// The text of a block's screen as the app last received it (None before any screen came).
    pub fn screen_text(&self, block_id: &str) -> Option<String> {
        self.app.model().screens.get(block_id).map(|f| f.text())
    }

    /// The target block and its screen (an empty one until the first arrives: keys typed by then
    /// still go to the program).
    fn target_screen(&self) -> Option<(String, String, Arc<render::Frame>)> {
        let (sid, id, _) = self.target()?;
        let f = self.app.model().screens.get(&id).cloned().unwrap_or_default();
        Some((sid, id, f))
    }

    /// (session id, target block id, whether it is zoomed) of the selected session's active window.
    fn target(&self) -> Option<(String, String, bool)> {
        let sid = self.session_id.clone()?;
        let m = self.app.model();
        let s = m.session(&sid)?;
        let w = s.windows.get(s.active)?;
        Some((sid, target(w)?.to_string(), w.zoomed.is_some()))
    }

    /// Keep the selection on a live session: one that ended (closed here, its shell exited, or
    /// killed from the CLI, or its host went away) hands over to the session now in its place.
    fn settle(&mut self, rows: &[SessionRow]) {
        if let Some(i) = rows.iter().position(|r| Some(&r.id) == self.session_id.as_ref()) {
            self.selected_at = i;
            self.use_host(rows[i].host);
            return;
        }
        self.selected_at = self.selected_at.min(rows.len().saturating_sub(1));
        self.session_id = rows.get(self.selected_at).map(|r| r.id.clone());
        self.use_host(rows.get(self.selected_at).map_or(0, |r| r.host));
    }

    /// Run a layout method on the selected session; redraw.
    fn call(&mut self, method: &str, mut params: serde_json::Value, cx: &mut Context<Self>) {
        let Some(sid) = self.session_id.clone() else { return };
        params["session_id"] = json!(sid);
        self.app.act_later(method, params, drop);
        let rows = self.rows();
        self.settle(&rows);
        cx.notify();
    }
}

#[path = "view_activity.rs"]
mod activity;
#[path = "view_bars.rs"]
mod bars;
#[path = "view_calls.rs"]
mod calls;
#[path = "view_copy.rs"]
mod copy;
#[path = "view_find.rs"]
mod find;
#[path = "view_floats.rs"]
mod floats_view;
#[path = "view_handlers.rs"]
mod handlers;
#[path = "view_header.rs"]
mod header;
#[path = "view_hosts.rs"]
mod hosts_view;
#[path = "view_keys.rs"]
mod keys;
#[path = "view_look.rs"]
mod look_view;
#[path = "view_start.rs"]
mod start;
pub(crate) use bars::Tip;
pub use look_view::is_dark;
pub use start::{init, install, quits, run};
#[path = "view_menus.rs"]
mod menus;
#[path = "view_model.rs"]
mod model;
#[path = "view_overlay.rs"]
mod overlay;
#[path = "view_paint.rs"]
mod paint;
#[path = "view_panes.rs"]
mod panes_view;
#[path = "view_scroll.rs"]
mod scroll;
#[path = "view_select.rs"]
mod select;
#[path = "view_sidebar.rs"]
mod sidebar;
#[path = "view_stats.rs"]
mod stats;
#[path = "view_switch.rs"]
mod switch;
use bars::status_mark;
pub use menus::{about_text, menus, set_version};
use paint::*;
use panes_view::*;
use stats::Stats;
