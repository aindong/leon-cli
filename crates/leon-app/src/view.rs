//! The window: a sidebar of sessions (with their tabs as rows, or in top tab style only the
//! sessions, the tabs then in the titlebar), and the active window's split layout as tiles, each a
//! pane header over a terminal painted from the screen the server pushes. Each machine's model is
//! an entity the reader thread's wake commits on the main thread; the workspace subscribes and
//! notifies only the views whose drawing changed (view_feed.rs).

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
use crate::program_kind::Kind;
use crate::remote::Remote;
use crate::render::{self, Frame};
use crate::status::{self, Indicator};
use crate::switcher;
use crate::theme::{Theme, SECONDARY_TEXT, UI_TEXT};
use section_inputs::{HeaderInput, HostLabel, SidebarInput, TitleBarInput};

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
/// The spinner's frame time: at most 8 redraws a second for it.
const SPIN: Duration = Duration::from_millis(125);
/// The largest paste sent: one request line is at most 1 MiB (`serve::MAX_LINE`), and JSON can
/// double a text's size.
const MAX_PASTE: usize = 400 << 10;

/// The block input goes to.
use crate::floats::target;

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

pub struct Workspace {
    /// The selected session's machine's remote: what every action acts on.
    app: Rc<Remote>,
    /// This Mac and the hosts; `host` is the one `app` belongs to.
    hosts: Hosts,
    host: usize,
    /// A session to select once it shows in its machine's model (New Session).
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
    /// How many times the wake handler ran, and how many times the workspace was notified (tests).
    wakes: (u64, u64),
    /// Each connected machine's model entity (view_feed.rs).
    models: Vec<(Rc<Remote>, feed::ModelSlot)>,
    /// What the root drew from the model and the wakes when last notified.
    root_seen: feed::RootInput,
    wake_seen: feed::WakeInput,
    /// The timed things, each a task of its own.
    deadlines: feed::Deadlines,
    /// Frames drawn (a clock that pauses while none are drawn compares it).
    frames: u64,
    /// The notifications posted, newest last.
    posted: Vec<crate::attention::Notice>,
    /// A tab or pane picked whose call is on its way (view_state.rs).
    asked: state::Asked,
    /// The overlays' focus handles and views (view_overlays.rs).
    overlays: overlays::Overlays,
    /// How many times the window rendered, and `prepare` ran (tests: what a frame did, R5).
    renders: u64,
    prepares: u64,
    keymap: Keymap,
    inbox: Arc<Mutex<poll_view::KeymapInbox>>,
    keymap_asked: Option<Instant>,
    /// Apply the next keymap reply (and fetch its settings) even if its generation didn't move:
    /// the last try failed.
    keymap_again: bool,
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
    /// Scrollback, selection, find and links per pane.
    term: scroll::TermUi,
    floats: floats_view::FloatUi,
    /// Each shown pane's grid view (view_grid.rs), and how often each rendered.
    grids: HashMap<String, grid_view::GridSlot>,
    grid_renders: Rc<RefCell<HashMap<String, u64>>>,
    /// The sidebar, title bar and headers as views (view_sections.rs), and how often each rendered.
    sections: HashMap<String, sections_view::SectionSlot>,
    section_renders: Rc<RefCell<HashMap<String, u64>>>,
    /// The spinners' clock, whether a shown one works, and the root's watch while it draws one.
    pulse: gpui::Entity<pulse_view::Pulse>,
    pulse_wanted: bool,
    pulse_root: Option<Subscription>,
    /// The pane under the mouse (its header shows its controls), and where the panes were.
    hovered_pane: Option<String>,
    hover_rects: Vec<(String, panes::Rect, &'static str)>,
    /// Each shown pane's fade (view_fade.rs).
    fade: fade_view::FadeUi,
    layout: layout_view::LayoutUi,
    /// The translucent window (view_glass.rs).
    glass: glass_view::GlassUi,
    overview: overview_view::OverviewUi,
    /// Leon.app's updates (view_update.rs): the notice, and the look for a release.
    updates: Arc<crate::updates::Updater>,
    /// Look on its own (the app; a test only when it asks), once the settings say whether to.
    updates_auto: bool,
    check_for_updates: Option<bool>,
    updates_looked: Option<Instant>,
    /// The cursor shader init.lua names, and the window's overlay for it.
    shader: cursor_shader_view::ShaderUi,
    /// Wakes the window from another thread (view_feed.rs).
    wake: Arc<dyn Fn() + Send + Sync>,
    _wakes: Task<()>,
    _clocks: (Task<()>, Option<Task<()>>),
    _activation: Subscription,
    _bounds: Subscription,
    _quit: Subscription,
    _release: Subscription,
    _appearance: Subscription,
    _prepare: Subscription,
    _refocus: Subscription,
}

#[path = "view_activity.rs"]
mod activity;
#[path = "view_bars.rs"]
mod bars;
#[path = "view_calls.rs"]
mod calls;
#[path = "view_copy.rs"]
mod copy;
#[path = "view_cursor_shader.rs"]
mod cursor_shader_view;
#[path = "view_drawn.rs"]
mod drawn_view;
#[path = "view_fade.rs"]
mod fade_view;
#[path = "view_feed.rs"]
mod feed;
#[path = "view_find.rs"]
mod find;
#[path = "view_floats.rs"]
mod floats_view;
#[path = "view_glass.rs"]
mod glass_view;
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
#[path = "view_layout.rs"]
mod layout_view;
#[path = "view_link_card.rs"]
pub mod link_card;
#[path = "view_look.rs"]
mod look_view;
#[path = "view_pulse.rs"]
mod pulse_view;
#[path = "view_section_inputs.rs"]
mod section_inputs;
#[path = "view_sections.rs"]
mod sections_view;
#[path = "view_start.rs"]
mod start;
#[path = "view_state.rs"]
mod state;
#[path = "view_theme.rs"]
mod theme_view;
pub(crate) use bars::Tip;

// the Settings window draws translucent as this window does
pub(crate) use floats_view::system_reduce_transparency;
pub(crate) use glass_view::set_blur_material;

pub use drawn_view::{drawing, drawing_cached, drawn, drawn_cached, sel};
pub use grid_view::GridDrawn;
pub use look_view::is_dark;
pub use pulse_view::Pulse;
pub use start::{init, install, quits, run};
pub use state::settings_path;
#[path = "view_menus.rs"]
mod menus;
#[path = "view_model.rs"]
mod model;
use model::{SessionRow, WindowRow};
#[path = "view_links.rs"]
mod links;
#[path = "view_new.rs"]
mod new;
#[path = "view_overlay.rs"]
mod overlay;
#[path = "view_overlays.rs"]
mod overlays;
use overlays::OverlayKind;
#[path = "view_overview.rs"]
mod overview_view;
#[path = "view_paint.rs"]
mod paint;
#[path = "view_panes.rs"]
mod panes_view;
#[path = "view_poll.rs"]
mod poll_view;
#[path = "view_presence.rs"]
mod presence;
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
use bars::{program_mark, status_mark};
pub use menus::{about_text, menus, set_version};
use paint::*;
use panes_view::*;
use stats::Stats;
