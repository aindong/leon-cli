//! M22b criterion 5: a setting init.lua sets by code `leon config set` can't rewrite shows
//! read-only in the Settings window, with the same `file:line` message, and is never written.
mod m9_support;
use gpui::{TestAppContext, VisualTestContext, WindowHandle};
use leon_app::settings_window::{Row, SettingsWindow};
use leon_lua::rewrite::{read_only, write_setting};
use m9_support::*;
use serde_json::json;

const SRC: &str = "local n = 15 leon.set(\"font.size\", n)\nleon.set(\"copy_on_select\", false)\n";

#[test]
fn read_only_says_what_config_set_says() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("init.lua");
    std::fs::write(&config, SRC).unwrap();
    let cfg = leon_lua::load(&config).unwrap();
    let why = read_only(&config, SRC, &cfg.setting_sources, "font.size").expect("font.size is read-only");
    let refused = write_setting(&config, "font.size", &json!(16)).unwrap_err();
    assert_eq!(why, refused, "one message for both");
    assert!(why.contains("init.lua:1"), "{why}");
    assert_eq!(read_only(&config, SRC, &cfg.setting_sources, "copy_on_select"), None, "a literal is writable");
    assert_eq!(read_only(&config, SRC, &cfg.setting_sources, "theme"), None, "an unset key is appended");
    assert_eq!(std::fs::read_to_string(&config).unwrap(), SRC);
}

#[gpui::test]
fn the_window_shows_it_read_only(cx: &mut TestAppContext) {
    let s = setup_with_config(SRC);
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.sync_keymap());
    cx.simulate_keystrokes("cmd-,");
    cx.run_until_parked();
    let sw: WindowHandle<SettingsWindow> =
        cx.windows().into_iter().find_map(|w| w.downcast::<SettingsWindow>()).expect("a Settings window");
    let row =
        |cx: &mut VisualTestContext, key: &str| -> Option<Row> { sw.read_with(cx, |sw, _| sw.row(key)).ok().flatten() };
    assert!(shows(cx, |cx| row(cx, "font.size").is_some_and(|r| r.read_only.is_some())), "font.size is read-only");
    let r = row(cx, "font.size").unwrap();
    assert_eq!(r.value, json!(15.0), "it still shows the value");
    assert!(r.read_only.unwrap().contains("init.lua:1"));
    assert_eq!(row(cx, "copy_on_select").unwrap().read_only, None);

    sw.update(cx, |sw, _, cx| sw.choose("font.size", json!(16), cx)).unwrap();
    assert!(!sw.read_with(cx, |sw, _| sw.saving()).unwrap(), "no write started");
    cx.run_until_parked();
    let text = std::fs::read_to_string(s.dir.path().join("init.lua")).unwrap();
    s.app.stop();
    assert_eq!(text, SRC, "a read-only control writes nothing");
}

#[test]
fn a_key_set_in_a_required_file_is_read_only() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("init.lua");
    std::fs::write(dir.path().join("more.lua"), "leon.set(\"copy_on_select\", true)\n").unwrap();
    let src = "require(\"more\")\n";
    std::fs::write(&config, src).unwrap();
    let cfg = leon_lua::load(&config).unwrap();
    assert!(cfg.settings.copy_on_select, "more.lua loaded: {:?}", cfg.diagnostics);
    let why = read_only(&config, src, &cfg.setting_sources, "copy_on_select").expect("read-only");
    assert!(why.contains("more.lua:1") && why.contains("a file other than the config"), "{why}");
}
