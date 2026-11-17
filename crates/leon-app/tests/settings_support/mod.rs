//! What the Settings window tests share: a private server with an init.lua, the Settings window
//! opened on it with a test context of its own, clicks by debug selector, and init.lua as
//! `leon config set` would leave it.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::rc::Rc;

use gpui::{px, size, Modifiers, TestAppContext, VisualTestContext, WindowHandle};
use leon_app::settings_window::SettingsWindow;
use serde_json::Value;

pub use super::m9_support::shows;
use super::m9_support::*;

pub struct Opened {
    pub s: Setup,
    pub sw: WindowHandle<SettingsWindow>,
    pub config: PathBuf,
}

/// A server on `src` and a Settings window on its init.lua, once the window has read it.
pub fn open_settings(cx: &mut TestAppContext, src: &str) -> (Opened, VisualTestContext) {
    let s = setup_with_config(src);
    let config = s.dir.path().join("init.lua");
    let remote = Rc::new(leon_app::remote::Remote::connect(s.app.socket()).expect("attached"));
    let at = config.clone();
    let sw = cx.update(|cx| SettingsWindow::open(at, remote, cx)).expect("Settings opened");
    let mut vcx = VisualTestContext::from_window(sw.into(), cx);
    // tall enough that every section's rows are on screen, so each control can be clicked
    vcx.simulate_resize(size(px(900.), px(2400.)));
    let o = Opened { s, sw, config };
    assert!(shows(&mut vcx, |cx| row(cx, &o, "theme").is_some()), "the window read init.lua");
    (o, vcx)
}

/// `key` as the window shows it.
pub fn row(cx: &mut VisualTestContext, o: &Opened, key: &str) -> Option<leon_app::settings_window::Row> {
    o.sw.read_with(cx, |sw, _| sw.row(key)).ok().flatten()
}

pub fn value(cx: &mut VisualTestContext, o: &Opened, key: &str) -> Option<Value> {
    row(cx, o, key).map(|r| r.value)
}

/// Click the element drawn with debug selector `sel`.
pub fn click(cx: &mut VisualTestContext, sel: &'static str) {
    cx.run_until_parked();
    let b = cx.debug_bounds(sel).unwrap_or_else(|| panic!("nothing drawn as {sel}"));
    cx.simulate_click(b.center(), Modifiers::none());
    cx.run_until_parked();
}

/// Whether something is drawn as `sel` now.
pub fn drawn(cx: &mut VisualTestContext, sel: &'static str) -> bool {
    cx.run_until_parked();
    cx.debug_bounds(sel).is_some()
}

/// Wait for the window's writes to end.
pub fn saved(cx: &mut VisualTestContext, o: &Opened) -> bool {
    shows(cx, |cx| !o.sw.read_with(cx, |sw, _| sw.saving()).unwrap())
}

/// The text `leon config set KEY VALUE` makes of init.lua `src`, written in a dir of its own.
pub fn config_set_text(src: &str, key: &str, value: &Value) -> String {
    let d = tempfile::tempdir().unwrap();
    let config = d.path().join("init.lua");
    std::fs::write(&config, src).unwrap();
    leon_lua::rewrite::write_setting(&config, key, value).expect("leon config set takes it");
    std::fs::read_to_string(&config).unwrap()
}

pub fn text(p: &Path) -> String {
    std::fs::read_to_string(p).unwrap()
}
