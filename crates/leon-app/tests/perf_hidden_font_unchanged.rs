//! A font setting that didn't change refits nothing: init.lua reloaded with the same font, or cmd+0
//! at the size the font already has, leaves a pane of a hidden session as it was.
mod m9_support;
use gpui::TestAppContext;
use leon_app::theme::LIGHT;
use m9_support::*;
use serde_json::json;

#[gpui::test]
fn an_unchanged_font_refits_nothing(cx: &mut TestAppContext) {
    let s = setup_with_config("leon.set(\"font.size\", 13)\nleon.set(\"theme\", \"leon-dark\")\n");
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.sync_keymap());
    let (_, block) = hidden_session(&s.app, &ws, cx);
    let hidden = term(&s.app, &block);
    assert_eq!(hidden.size(), (24, 80), "a pane never shown has the server's default size");

    // a reload that changes the theme and leaves the font
    leon_lua::rewrite::write_setting(&s.dir.path().join("init.lua"), "theme", &json!("leon-light")).unwrap();
    s.app.call("server.config_reload", json!({})).unwrap();
    ws.update(cx, |ws, cx| {
        ws.sync_keymap();
        cx.notify();
    });
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.theme()) == LIGHT), "the reload applied");
    let after_reload = stays(cx, &hidden, (24, 80));

    cx.simulate_keystrokes("cmd-0");
    let after_reset = stays(cx, &hidden, (24, 80));
    let size = hidden.size();
    s.app.stop();
    assert!(after_reload, "a reload with the same font refits nothing: {size:?}");
    assert!(after_reset, "cmd+0 at the size the font has refits nothing: {size:?}");
}
