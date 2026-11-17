//! ADR R3/R4 (docs/architecture/gpui.md), phase 3: how see-through the window is (glass.rs) is in
//! the sidebar's and the title bar's inputs, since both fill with `chrome_fill`: a change of it
//! renders them again.
mod grid_support;

use gpui::{TestAppContext, VisualTestContext};
use grid_support::*;
use leon_lua::rewrite::write_setting;
use serde_json::json;

/// Longer than the app waits between two looks at the config.
const POLL: std::time::Duration = std::time::Duration::from_millis(1100);

/// Leave the window and come back to it, as after changing System Settings.
fn come_back(cx: &mut VisualTestContext) {
    cx.deactivate_window();
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
}

#[gpui::test]
fn the_sidebar_and_title_bar_render_on_the_glass(cx: &mut TestAppContext) {
    let s = setup_with_config("leon.set(\"translucency\", 0.3)\n");
    let (ws, cx) = open(cx, &s.app);
    act(&ws, cx, "client.tab_style.toggle"); // top tabs: the title bar shows beside the sidebar
    ws.update(cx, |ws, _| ws.set_reduce_transparency(Some(false)));
    come_back(cx);
    let clear = shows(cx, |cx| ws.read_with(cx, |ws, _| ws.glass().chrome_alpha() < 1.));
    assert!(clear, "0.3 lets the chrome show through");
    let (sidebar, titlebar) = (section_renders(&ws, cx, "sidebar"), section_renders(&ws, cx, "titlebar"));
    write_setting(&s.dir.path().join("init.lua"), "translucency", &json!(0.)).unwrap();
    s.app.call("server.config_reload", json!({})).unwrap();
    let opaque = shows(cx, |cx| {
        cx.executor().advance_clock(POLL);
        cx.run_until_parked();
        ws.read_with(cx, |ws, _| ws.glass().chrome_alpha() == 1.)
    });
    let again = shows(cx, |cx| {
        section_renders(&ws, cx, "sidebar") > sidebar && section_renders(&ws, cx, "titlebar") > titlebar
    });
    s.app.stop();
    assert!(opaque, "0 makes the chrome opaque");
    assert!(again, "the sidebar and the title bar drew the opaque fill");
}
