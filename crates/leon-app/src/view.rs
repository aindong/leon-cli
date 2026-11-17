//! The window: a sidebar of sessions (with their tabs as rows, or in top tab style only the
//! sessions, the tabs then in the titlebar), and the active window's split layout as tiles, each a
//! pane header over a terminal painted from the screen the server pushes. A task
//! on the GPUI executor redraws when the model or a screen changed, woken by the reader thread
//! or at a deadline while something timed is pending (view_poll.rs).

use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
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
use sections_view::{HeaderInput, HostLabel, SidebarInput, TitleBarInput};

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
/// The poll's pace while something moves on screen (a slide, a fade): about 120 Hz.
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
#[derive(Clone, PartialEq, Debug)]
struct SessionRow {
    /// Which machine it is on (`Hosts` index; 0 is This Mac).
    host: usize,
    id: String,
    title: String,
    windows: Vec<WindowRow>,
    active: usize,
    status: Option<Indicator>,
}

#[derive(Clone, PartialEq, Debug)]
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
    /// Why the last ⌘↩ couldn't split (another machine's row), or a theme wasn't kept.
    note: Option<String>,
    /// The theme list: every theme; the selected one is drawn while it is open.
    themes: Option<Vec<(String, Theme)>>,
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

// Truncating text cells use `line_clamp(1)`, not `whitespace_nowrap`: GPUI 0.2.2 caches a no-wrap
// text's size from its first layout (before its width is known) and never adds the ellipsis.

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
    /// How many times the poll has run, and asked for a redraw (tests: it runs when woken or when
    /// something timed is due).
    polls: (u64, u64),
    /// How many times the window rendered, and `prepare` ran (tests: what a frame did, R5).
    renders: u64,
    prepares: u64,
    /// Redraws the poll asked for since the last frame was drawn: a window that is hidden or
    /// covered draws none, and from the second the poll stops the clocks only a frame shows.
    undrawn: u8,
    /// When the poll looks next without a wake (None: only a wake).
    planned: Option<Instant>,
    keymap: Keymap,
    inbox: Arc<Mutex<poll_view::KeymapInbox>>,
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
    /// The native key event being handled, as (key code, text): `input::current_key_event` in the
    /// real app, a stand-in in tests, none otherwise.
    native_key: fn() -> Option<(u16, String)>,
    /// This Mac's server went away from outside (`leon server stop`): the window says so.
    stopped: bool,
    /// The app stops the server itself (Quit and End All Sessions): its going is a quit.
    ending: bool,
    /// What Start It Again would run, in a test workspace (which never spawns it).
    relaunched: Option<Vec<String>>,
    /// The window is the key window: the user is looking at leon.
    window_active: bool,
    font_size: f32,
    /// init.lua's settings as drawn (`look.rs`); the theme for the OS appearance now (`dark`),
    /// and the terminal font.
    look: Look,
    dark: bool,
    /// The colours this frame draws with (`themes`: a change fading in).
    theme: Theme,
    themes: theme_view::ThemeUi,
    font_family: SharedString,
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
    /// The focus event `note_focus` found after a frame, for the poll to send, on its pane's machine.
    focus_event: Option<(Rc<Remote>, Value)>,
    /// Scrollback, selection, find and links per pane.
    term: scroll::TermUi,
    floats: floats_view::FloatUi,
    /// Each shown pane's grid view (view_grid.rs), and how often each rendered.
    grids: HashMap<String, grid_view::GridSlot>,
    grid_renders: Rc<RefCell<HashMap<String, u64>>>,
    /// The sidebar, title bar and headers as views (view_sections.rs), and how often each rendered.
    sections: HashMap<String, sections_view::SectionSlot>,
    section_renders: Rc<RefCell<HashMap<String, u64>>>,
    /// The spinners' clock, whether a shown indicator works, and the root observing it while it
    /// draws one itself (the overview's cards).
    pulse: gpui::Entity<pulse_view::Pulse>,
    pulse_wanted: bool,
    pulse_root: Option<Subscription>,
    /// The pane under the mouse: its header shows its controls.
    hovered_pane: Option<String>,
    /// Each shown pane's fade (view_fade.rs).
    fade: fade_view::FadeUi,
    overview: overview_view::OverviewUi,
    /// Leon.app's updates (view_update.rs): the notice, and the look for a release.
    updates: Arc<crate::updates::Updater>,
    /// Look on its own (the app; a test only when it asks), once the settings say whether to.
    updates_auto: bool,
    check_for_updates: Option<bool>,
    updates_looked: Option<Instant>,
    /// Wakes the poll from another thread.
    wake: Arc<dyn Fn() + Send + Sync>,
    _poll: Task<()>,
    _tick: Task<()>,
    _activation: Subscription,
    _bounds: Subscription,
    _quit: Subscription,
    _release: Subscription,
    _appearance: Subscription,
    _prepare: Subscription,
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
        ws.native_key = crate::input::current_key_event;
        let store = leon_core::paths::state_dir().map(|d| d.join("updates.json"));
        ws.enable_updates(store, Arc::new(leon_update::Target::running));
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
        let (wake, woken) = futures::channel::mpsc::channel::<()>(0);
        let wake = Mutex::new(wake);
        let wake: Arc<dyn Fn() + Send + Sync> =
            Arc::new(move || drop(wake.lock().unwrap_or_else(|e| e.into_inner()).try_send(())));
        let woke = wake.clone();
        app.on_change(move || woke());
        crate::notify::on_click(&wake);
        let term = scroll::TermUi { wake: Some(wake.clone()), ..Default::default() };
        let updates =
            Arc::new(crate::updates::Updater::new(None, Arc::new(leon_update::Target::running), wake.clone()));
        let hosts = Hosts::new(app.clone(), hosts, wake.clone());
        let (poll, tick) = Self::start_poll(woken, cx);
        let stats = Stats::new();
        let session_id = app.model().sessions.first().map(|s| s.id.clone());
        let activation = cx.observe_window_activation(window, |ws, window, cx| {
            ws.window_active = window.is_window_active();
            cx.notify()
        });
        // the window's frame and display are kept with the settings, and saved when the app quits
        // or the window closes (closing drops the workspace before the quit it asks for)
        let bounds = cx.observe_window_bounds(window, |ws, window, cx| {
            ws.settings.window = Some(state::frame(window.window_bounds().get_bounds()));
            ws.settings.display = window.display(cx).map(|d| u32::from(d.id()));
            cx.notify(); // the headers' titles are cut to the panes' widths
        });
        let quit = cx.on_app_quit(|ws, _| {
            ws.save_settings();
            async {}
        });
        let release = cx.on_release(|ws, _| ws.save_settings());
        let appearance = cx.observe_window_appearance(window, |ws, window, cx| {
            ws.set_appearance(look_view::is_dark(window.appearance()), cx)
        });
        // shortcut: every notify of the workspace brings what render draws from up to date (R5),
        // and gives each grid and section its input; a pushed screen reaches its grid from the poll
        // instead (`poll_grids`). Phase 5 (the model as entities) removes this observer
        let prepare = cx.observe_in(&cx.entity(), window, |ws, _, window, cx| {
            let seen = ws.screens_seen();
            ws.prepare();
            ws.sync_grids(&seen, cx);
            ws.sync_sections(window, cx);
        });
        let (look, dark) = (Look::default(), look_view::is_dark(window.appearance()));
        let mut ws = Workspace {
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
            polls: (0, 0),
            renders: 0,
            prepares: 0,
            undrawn: 0,
            planned: None,
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
            native_key: || None,
            stopped: false,
            ending: false,
            relaunched: None,
            window_active: window.is_window_active(),
            font_size: look.font_size,
            theme: look.theme(dark),
            themes: theme_view::ThemeUi::new(look.theme(dark)),
            font_family: look.font_family.clone().into(),
            look,
            dark,
            config_path: None,
            settings_window: None,
            epoch: Instant::now(),
            seen_focus: None,
            focus_event: None,
            term,
            floats: Default::default(),
            grids: HashMap::new(),
            grid_renders: Rc::default(),
            sections: HashMap::new(),
            section_renders: Rc::default(),
            pulse: cx.new(|_| pulse_view::Pulse::new()),
            pulse_wanted: false,
            pulse_root: None,
            hovered_pane: None,
            fade: Default::default(),
            overview: Default::default(),
            updates,
            updates_auto: false,
            check_for_updates: None,
            updates_looked: None,
            wake,
            _poll: poll,
            _tick: tick,
            _activation: activation,
            _bounds: bounds,
            _quit: quit,
            _release: release,
            _appearance: appearance,
            _prepare: prepare,
        };
        let seen = ws.screens_seen();
        ws.prepare(); // the first frame comes before any notify
        ws.sync_grids(&seen, cx);
        ws.sync_sections(window, cx);
        ws
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
#[path = "view_drawn.rs"]
mod drawn_view;
#[path = "view_fade.rs"]
mod fade_view;
#[path = "view_find.rs"]
mod find;
#[path = "view_floats.rs"]
mod floats_view;
#[path = "view_grid.rs"]
mod grid_view;
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
#[path = "view_pulse.rs"]
mod pulse_view;
#[path = "view_sections.rs"]
mod sections_view;
#[path = "view_start.rs"]
mod start;
#[path = "view_state.rs"]
mod state;
#[path = "view_theme.rs"]
mod theme_view;
pub(crate) use bars::Tip;
pub use drawn_view::{drawing, drawing_cached, drawn, drawn_cached, sel};
pub use grid_view::GridDrawn;
pub use look_view::is_dark;
pub use pulse_view::Pulse;
pub use start::{init, install, quits, run};
#[path = "view_menus.rs"]
mod menus;
#[path = "view_model.rs"]
mod model;
#[path = "view_overlay.rs"]
mod overlay;
#[path = "view_overview.rs"]
mod overview_view;
#[path = "view_paint.rs"]
mod paint;
#[path = "view_panes.rs"]
mod panes_view;
#[path = "view_poll.rs"]
mod poll_view;
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
#[path = "view_update.rs"]
mod update_view;
use bars::status_mark;
pub use menus::{about_text, menus, set_version};
use paint::*;
use panes_view::*;
use stats::Stats;
