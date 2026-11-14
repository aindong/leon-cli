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

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub tab_style: TabStyle,
    /// Pane headers shown (`client.pane_header.reveal` toggles them).
    pub pane_headers: bool,
    pub sidebar_width: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { tab_style: TabStyle::Sidebar, pane_headers: true, sidebar_width: 220. }
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
        let dir = path.parent().ok_or_else(|| std::io::Error::other("no dir"))?;
        leon_core::paths::ensure_private_dir(dir)?;
        let tmp = path.with_extension("json.tmp");
        let mut f = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&tmp)?;
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
