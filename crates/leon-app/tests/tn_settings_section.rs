//! Serve over Tailscale, the Settings window's part: its switch writes `serve_tailscale` through
//! `leon config set`'s code, and the section follows `server.status`'s `tailnet`. The seam points
//! the server at a LocalAPI that isn't there, so it never reaches this machine's Tailscale.
mod m9_support;
use gpui::{TestAppContext, VisualTestContext, WindowHandle};
use leon_app::settings_window::SettingsWindow;
use m9_support::*;
use serde_json::json;

#[gpui::test]
fn the_switch_turns_serving_on_and_the_section_says_how_it_went(cx: &mut TestAppContext) {
    let nowhere = tempfile::tempdir().unwrap();
    // the only test in this binary: no other test sees the seam
    std::env::set_var("LEON_TEST_TAILNET", json!({ "localapi": nowhere.path().join("none.sock") }).to_string());
    let s = setup_with_config("");
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.sync_keymap());
    cx.simulate_keystrokes("cmd-,");
    cx.run_until_parked();
    let sw: WindowHandle<SettingsWindow> =
        cx.windows().into_iter().find_map(|w| w.downcast::<SettingsWindow>()).expect("a Settings window");
    let state = |cx: &mut VisualTestContext| {
        sw.read_with(cx, |sw, _| sw.tailnet()).ok().flatten().map(|t| t["state"].as_str().unwrap_or("").to_string())
    };
    assert!(shows(cx, |cx| state(cx).as_deref() == Some("off")), "off by default: {:?}", state(cx));

    sw.update(cx, |sw, _, cx| sw.choose("serve_tailscale", json!(true), cx)).unwrap();
    let stopped = |cx: &mut VisualTestContext| {
        cx.executor().advance_clock(std::time::Duration::from_secs(2)); // the window's 2 s poll
        state(cx).as_deref() == Some("stopped")
    };
    assert!(shows(cx, stopped), "the section follows the server: {:?}", state(cx));
    let t = sw.read_with(cx, |sw, _| sw.tailnet()).unwrap().unwrap();
    let text = std::fs::read_to_string(s.dir.path().join("init.lua")).unwrap();
    s.app.stop();
    assert!(t["reason"].as_str().unwrap_or("").contains("LocalAPI"), "{t}");
    assert!(text.contains("leon.set(\"serve_tailscale\", true)"), "{text}");
}
