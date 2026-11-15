//! M22b criterion 4: ⌘, opens one Settings window whose controls write init.lua through
//! `leon config set`'s code, keeping the rest of the file, and the server follows.
mod m9_support;
use gpui::{TestAppContext, VisualTestContext, WindowHandle};
use leon_app::settings_window::SettingsWindow;
use m9_support::*;
use serde_json::{json, Value};

fn settings_windows(cx: &VisualTestContext) -> Vec<WindowHandle<SettingsWindow>> {
    cx.windows().into_iter().filter_map(|w| w.downcast::<SettingsWindow>()).collect()
}

/// The Settings window's idea of `key`, once it has one.
fn shown(cx: &mut VisualTestContext, sw: WindowHandle<SettingsWindow>, key: &str) -> Option<Value> {
    sw.read_with(cx, |sw, _| sw.row(key).map(|r| r.value)).ok().flatten()
}

#[gpui::test]
fn the_window_writes_init_lua(cx: &mut TestAppContext) {
    let src = "-- mine\nlocal greeting = \"hi\"\nleon.set(\"font.size\", 13) -- keep this\n";
    let s = setup_with_config(src);
    let config = s.dir.path().join("init.lua");
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.sync_keymap());
    cx.simulate_keystrokes("cmd-,");
    cx.run_until_parked();
    cx.simulate_keystrokes("cmd-,");
    cx.run_until_parked();
    let all = settings_windows(cx);
    assert_eq!(all.len(), 1, "a second ⌘, keeps the one window");
    let sw = all[0];
    assert!(shows(cx, |cx| shown(cx, sw, "font.size") == Some(json!(13.0))), "the window shows init.lua's size");

    let choose = |cx: &mut VisualTestContext, key: &str, v: Value| {
        sw.update(cx, |sw, _, cx| sw.choose(key, v.clone(), cx)).unwrap();
        assert!(shows(cx, |cx| shown(cx, sw, key) == Some(v.clone())), "{key} never showed {v}");
    };
    choose(cx, "font.size", json!(15.0));
    let after = std::fs::read_to_string(&config).unwrap();
    assert_eq!(after, src.replace("13)", "15)"), "only the value changed");

    choose(cx, "theme", json!({ "dark": "leon-dark", "light": "leon-light" }));
    choose(cx, "font.family", json!("Monaco"));
    choose(cx, "notifications.blocked", json!(false));
    choose(cx, "copy_on_select", json!(true));

    let cfg = leon_lua::load(&config).unwrap();
    let server = s.app.call("server.settings", json!({})).unwrap();
    s.app.stop();
    let st = &cfg.settings;
    assert_eq!((st.theme.dark.as_str(), st.theme.light.as_str()), ("leon-dark", "leon-light"));
    assert_eq!((st.font_family.as_str(), st.font_size), ("Monaco", 15.));
    assert!(!st.notifications.blocked && st.copy_on_select);
    assert_eq!(server["settings"], st.to_json(), "the server reloaded what the window wrote");
}

#[gpui::test]
fn a_refused_value_shows_why_and_writes_nothing(cx: &mut TestAppContext) {
    let src = "leon.set(\"font.size\", 13)\n";
    let s = setup_with_config(src);
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.sync_keymap());
    cx.simulate_keystrokes("cmd-,");
    cx.run_until_parked();
    let sw = settings_windows(cx)[0];
    assert!(shows(cx, |cx| shown(cx, sw, "font.size").is_some()));
    sw.update(cx, |sw, _, cx| sw.choose("font.size", json!(99), cx)).unwrap();
    let error = |cx: &mut VisualTestContext| sw.read_with(cx, |sw, _| sw.error()).ok().flatten();
    assert!(shows(cx, |cx| error(cx).is_some()), "a refusal shows");
    let text = std::fs::read_to_string(s.dir.path().join("init.lua")).unwrap();
    s.app.stop();
    assert!(error(cx).unwrap().contains("font.size must be a number from 8 to 32"), "{:?}", error(cx));
    assert_eq!(text, src);
}
