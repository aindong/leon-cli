//! Where the window puts its tabs (one place only), and the app's own settings, kept across
//! launches in `$LEON_STATE_DIR/app.json`.

use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// Where tabs are drawn: as rows in the sidebar, or in the titlebar row (`client.tab_style.toggle`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TabStyle {
    #[default]
    Sidebar,
    Top,
}

impl TabStyle {
    pub fn toggled(self) -> TabStyle {
        match self {
            TabStyle::Sidebar => TabStyle::Top,
            TabStyle::Top => TabStyle::Sidebar,
        }
    }
}

/// What the sidebar lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarList {
    SessionsAndTabs,
    SessionsOnly,
    /// The Activity list (`client.sidebar.activity.toggle`) in place of the sessions.
    Activity,
}

/// Where the chrome goes for a tab style and whether the sidebar is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Places {
    pub sidebar: Option<SidebarList>,
    pub top_tabs: bool,
}

/// Tabs are drawn in one place: the sidebar in sidebar style (none while it is hidden: ⌘1…9 and
/// the palette still reach them), the titlebar in top style, where the sidebar lists sessions only.
pub fn places(style: TabStyle, sidebar_shown: bool) -> Places {
    let list = match style {
        TabStyle::Sidebar => SidebarList::SessionsAndTabs,
        TabStyle::Top => SidebarList::SessionsOnly,
    };
    Places { sidebar: sidebar_shown.then_some(list), top_tabs: style == TabStyle::Top }
}

/// The sidebar's width range in points; dragging its edge stays inside it.
pub const SIDEBAR_WIDTHS: (f32, f32) = (160., 400.);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub tab_style: TabStyle,
    /// Pane headers shown (`client.pane_header.reveal` toggles them).
    pub pane_headers: bool,
    pub sidebar_width: f32,
    /// The window's last frame (its restored size, when it was zoomed or full screen), relative to
    /// its display, and that display's id.
    pub window: Option<Frame>,
    pub display: Option<u32>,
    /// The machines whose sessions the sidebar hides, by label (`This Mac` for this one).
    pub collapsed_hosts: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            tab_style: TabStyle::Sidebar,
            pane_headers: true,
            sidebar_width: 220.,
            window: None,
            display: None,
            collapsed_hosts: Vec::new(),
        }
    }
}

impl Settings {
    /// The settings in `path`; a missing or unreadable file gives the defaults.
    pub fn load(path: &Path) -> Settings {
        let mut s: Settings =
            std::fs::read(path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        s.sidebar_width = clamp_width(s.sidebar_width);
        s
    }

    /// Write to `path` (0600, through a temp file and a rename) inside a private dir of ours: the
    /// check the server makes of the same state dir.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        use std::os::unix::fs::OpenOptionsExt;
        let dir = path.parent().filter(|d| d.is_dir()).ok_or_else(|| std::io::Error::other("no state dir"))?;
        leon_core::paths::ensure_private_dir(dir)?; // never created here: the server makes it
        let tmp = path.with_extension("json.tmp");
        let _ = std::fs::remove_file(&tmp); // a crash's leftover, so the new file gets 0600
        let mut f = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&tmp)?;
        f.write_all(&serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?)?;
        std::fs::rename(&tmp, path)
    }
}

pub fn clamp_width(w: f32) -> f32 {
    if w.is_finite() {
        w.clamp(SIDEBAR_WIDTHS.0, SIDEBAR_WIDTHS.1)
    } else {
        Settings::default().sidebar_width
    }
}

/// A window or screen rectangle in points.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// The smallest the window gets, and the smallest a first window opens at (when the screen fits it).
pub const MIN_WINDOW: (f32, f32) = (720., 440.);
const FIRST_WINDOW: (f32, f32) = (1000., 680.);

/// Where the window opens: the saved frame when at least half of it is on one of `screens` (the
/// primary first), else three quarters of the primary screen, centred.
pub fn window_frame(saved: Option<Frame>, screens: &[Frame]) -> Frame {
    let finite = |f: &Frame| [f.x, f.y, f.w, f.h].iter().all(|v| v.is_finite());
    if let Some(s) = saved.filter(finite) {
        let s = Frame { w: s.w.max(MIN_WINDOW.0), h: s.h.max(MIN_WINDOW.1), ..s };
        if screens.iter().any(|screen| 2. * overlap(&s, screen) >= s.w * s.h) {
            return s;
        }
    }
    let screen = screens.first().copied().unwrap_or(Frame { x: 0., y: 0., w: FIRST_WINDOW.0, h: FIRST_WINDOW.1 });
    let w = (screen.w * 0.75).max(FIRST_WINDOW.0).min(screen.w);
    let h = (screen.h * 0.75).max(FIRST_WINDOW.1).min(screen.h);
    Frame { x: screen.x + (screen.w - w) / 2., y: screen.y + (screen.h - h) / 2., w, h }
}

fn overlap(a: &Frame, b: &Frame) -> f32 {
    let w = (a.x + a.w).min(b.x + b.w) - a.x.max(b.x);
    let h = (a.y + a.h).min(b.y + b.h) - a.y.max(b.y);
    w.max(0.) * h.max(0.)
}

/// A directory too wide for its room, cut in the middle: its start (the root and the first
/// component, `~/src`) and its last two (`api/routes`) are what tell panes apart.
pub fn elide_path(path: &str, mut fits: impl FnMut(&str) -> bool) -> String {
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() <= 4 || fits(path) {
        return path.to_string();
    }
    format!("{}/…/{}", parts[..2].join("/"), parts[parts.len() - 2..].join("/"))
}
