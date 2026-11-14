//! What the M10b view tests share: the M9 helpers, a workspace on a settings file, and element checks.
#![allow(dead_code)]

#[path = "../m9_support/mod.rs"]
mod m9;
pub use m9::*;

use std::path::PathBuf;
use std::rc::Rc;

use gpui::{px, size, Bounds, Entity, Pixels, TestAppContext, VisualTestContext};
use leon_app::embedded::Embedded;
use leon_app::view::{self, Workspace};

/// A workspace that keeps its settings in `settings` (None: in memory only, as `Workspace::new`).
pub fn open_with<'a>(
    cx: &'a mut TestAppContext,
    app: &Rc<Embedded>,
    settings: Option<PathBuf>,
) -> (Entity<Workspace>, &'a mut VisualTestContext) {
    cx.update(view::init);
    let app = app.clone();
    let (ws, vcx) = cx.add_window_view(move |window, cx| {
        let remote = Rc::new(leon_app::remote::Remote::connect(app.socket()).expect("the app attaches"));
        Workspace::with_settings(remote, settings, window, cx)
    });
    vcx.simulate_resize(size(px(1400.), px(900.)));
    vcx.run_until_parked();
    (ws, vcx)
}

/// The bounds of the element drawn with debug selector `sel` in the last frame, if any. GPUI
/// keeps a selector's bounds after its element is gone, so the view's own record says what's drawn.
pub fn at(cx: &mut VisualTestContext, sel: &str) -> Option<Bounds<Pixels>> {
    cx.run_until_parked();
    if !view::drawn().contains(sel) {
        return None;
    }
    cx.debug_bounds(Box::leak(sel.to_string().into_boxed_str()))
}

pub fn drawn(cx: &mut VisualTestContext, sel: &str) -> bool {
    at(cx, sel).is_some()
}

/// Run an app action by name, as a key or the palette would.
pub fn act(ws: &Entity<Workspace>, cx: &mut VisualTestContext, name: &str) {
    cx.update(|window, cx| ws.update(cx, |ws, cx| ws.run_action(name, None, "test", window, cx)));
    cx.run_until_parked();
}

pub fn center(b: Bounds<Pixels>) -> gpui::Point<Pixels> {
    gpui::point(b.origin.x + b.size.width / 2., b.origin.y + b.size.height / 2.)
}

pub fn private_dir() -> tempfile::TempDir {
    use std::os::unix::fs::PermissionsExt;
    tempfile::Builder::new().permissions(std::fs::Permissions::from_mode(0o700)).tempdir().unwrap()
}
