//! The app's icons: one set of stroke SVGs (24-unit grid, one stroke weight, round caps), compiled
//! into the binary and served to GPUI as assets. GPUI draws an SVG as a mask in the text colour.

use std::borrow::Cow;

use gpui::{prelude::*, px, rgb, svg, AssetSource, SharedString, Svg};

/// Toolbar icons and their hit targets, and the icons in rows, tabs and pane headers, in points.
pub const TOOLBAR_ICON: f32 = 16.;
pub const TOOLBAR_TARGET: f32 = 28.;
pub const ROW_ICON: f32 = 14.;

macro_rules! icons {
    ($($name:ident => $file:literal),* $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum Icon { $($name),* }

        impl Icon {
            pub const ALL: &'static [Icon] = &[$(Icon::$name),*];

            /// Its asset path.
            pub fn path(self) -> &'static str {
                match self { $(Icon::$name => concat!("icons/", $file, ".svg")),* }
            }

            fn bytes(self) -> &'static [u8] {
                match self { $(Icon::$name => include_bytes!(concat!("../icons/", $file, ".svg"))),* }
            }
        }
    };
}

icons! {
    Sidebar => "sidebar",
    Command => "command",
    Grid => "grid",
    Plus => "plus",
    SplitRight => "split-right",
    SplitDown => "split-down",
    Zoom => "zoom",
    Unfloat => "unfloat",
    Close => "close",
    Terminal => "terminal",
    Search => "search",
    Working => "working",
    Done => "done",
    Blocked => "blocked",
    Question => "question",
    Permission => "permission",
    Auth => "auth",
    Error => "error",
}

impl Icon {
    /// The icon at `size` points in `color`: GPUI's `svg` paints only in a colour set on itself.
    pub fn at(self, size: f32, color: u32) -> Svg {
        svg().path(self.path()).size(px(size)).flex_none().text_color(rgb(color))
    }

    /// `at`, brightening to `hover` while the mouse is over `group` (the button around it).
    pub fn in_button(self, size: f32, color: u32, hover: u32, group: impl Into<SharedString>) -> Svg {
        self.at(size, color).group_hover(group, move |s| s.text_color(rgb(hover)))
    }
}

/// The asset source the app is built with: the icons, and nothing else.
pub struct Icons;

impl AssetSource for Icons {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        Ok(Icon::ALL.iter().find(|i| i.path() == path).map(|i| Cow::Borrowed(i.bytes())))
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
        Ok(Icon::ALL.iter().map(|i| i.path()).filter(|p| p.starts_with(path)).map(SharedString::from).collect())
    }
}
