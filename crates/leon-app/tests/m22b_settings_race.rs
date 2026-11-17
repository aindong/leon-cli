//! Two Settings clicks in a row, the second before the first has saved, both reach init.lua.
mod m9_support;
use gpui::{TestAppContext, VisualTestContext, WindowHandle};
use leon_app::settings_window::SettingsWindow;
use m9_support::*;
use serde_json::{json, Value};

#[gpui::test]
fn two_quick_theme_clicks_keep_both(cx: &mut TestAppContext) {
    let s = setup_with_config("-- mine\n");
    let config = s.dir.path().join("init.lua");
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, cx| ws.sync_keymap(cx));
    cx.simulate_keystrokes("cmd-,");
    cx.run_until_parked();
    let sw: WindowHandle<SettingsWindow> =
        cx.windows().into_iter().find_map(|w| w.downcast::<SettingsWindow>()).expect("Settings opened");
    let theme = |cx: &mut VisualTestContext| -> Option<Value> {
        sw.read_with(cx, |sw, _| sw.row("theme").map(|r| r.value)).ok().flatten()
    };
    assert!(shows(cx, |cx| theme(cx).is_some()), "the window read init.lua");
    // what a theme row's click sends: the side clicked, the other side as the window shows it now
    let click = |cx: &mut VisualTestContext, side: &str, name: &str| {
        let now = theme(cx).unwrap();
        let mut v = json!({ "dark": now["dark"], "light": now["light"] });
        v[side] = json!(name);
        sw.update(cx, |sw, _, cx| sw.choose("theme", v, cx)).unwrap();
    };
    click(cx, "dark", "leon-light");
    click(cx, "light", "leon-light"); // before the first change has saved
    let file = || leon_lua::load(&config).ok().map(|c| (c.settings.theme.dark, c.settings.theme.light));
    let both = shows(cx, |_| file() == Some(("leon-light".to_string(), "leon-light".to_string())));
    let got = file();
    s.app.stop();
    assert!(both, "a second click while the first was saving undid it: init.lua has {got:?}");
}
