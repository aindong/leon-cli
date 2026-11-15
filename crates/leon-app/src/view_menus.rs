//! The menu bar (moved as is from view.rs).

use super::*;

/// The version About shows, as `leon --version` prints it (`leon app` passes in `LEON_VERSION`,
/// which has the commit; this crate's build can't see it).
static VERSION: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// Set the version About shows (once; later calls are ignored).
pub fn set_version(v: &str) {
    let _ = VERSION.set(v.to_string());
}

/// What About says: `leon <version>`.
pub fn about_text() -> String {
    format!("leon {}", VERSION.get().map_or(env!("CARGO_PKG_VERSION"), String::as_str))
}

/// The menu bar. "Quit leon" (⌘Q) detaches and leaves every program running; ending them is the
/// separate item that asks first.
pub fn menus() -> Vec<Menu> {
    vec![
        Menu {
            name: "leon".into(),
            items: vec![
                MenuItem::action("About leon", About),
                MenuItem::separator(),
                MenuItem::action("Quit leon", Quit),
                MenuItem::action("Quit and End All Sessions…", QuitAndEndAll),
            ],
        },
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
        Menu {
            name: "Edit".into(),
            items: vec![
                MenuItem::action("Copy", Copy),
                MenuItem::action("Paste", Paste),
                MenuItem::action("Select All", SelectAll),
                MenuItem::separator(),
                MenuItem::action("Find…", Find),
            ],
        },
        Menu {
            name: "View".into(),
            items: vec![
                MenuItem::action("Toggle Sidebar", ToggleSidebar),
                MenuItem::separator(),
                MenuItem::action("Next Tab", NextTab),
                MenuItem::action("Previous Tab", PreviousTab),
            ],
        },
    ]
}
