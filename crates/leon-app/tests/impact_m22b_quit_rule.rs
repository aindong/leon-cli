//! M22b impact: the app quits when its window closes, as before, but not when Settings does.
//! GPUI drops a closing window from `windows()` before it runs the close observers
//! (gpui-0.2.2 app.rs:1375-1380), which `view::quits` relies on.
mod m9_support;
use gpui::TestAppContext;
use leon_app::settings_window::SettingsWindow;
use leon_app::view::quits;
use m9_support::*;

#[gpui::test]
fn closing_settings_keeps_the_app_closing_the_workspace_quits(cx: &mut TestAppContext) {
    let s = setup_with_config("");
    let (ws, vcx) = open(cx, &s.app);
    ws.update(vcx, |ws, _| ws.sync_keymap());
    vcx.simulate_keystrokes("cmd-,");
    vcx.run_until_parked();
    let settings = vcx.windows().into_iter().find_map(|w| w.downcast::<SettingsWindow>()).expect("Settings opened");
    assert!(!vcx.update(|_, cx| quits(cx)), "both windows open");
    settings.update(vcx, |_, window, _| window.remove_window()).unwrap();
    vcx.run_until_parked();
    let after_settings = vcx.update(|_, cx| quits(cx));
    drop(ws);
    vcx.update(|window, _| window.remove_window());
    vcx.run_until_parked();
    let after_workspace = cx.update(|cx| quits(cx));
    s.app.stop();
    assert!(!after_settings, "closing Settings leaves the app running");
    assert!(after_workspace, "closing the workspace window quits");
}
