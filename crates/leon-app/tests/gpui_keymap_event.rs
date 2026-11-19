//! GPUI phase 5: a config reload reaches the window through the server's `keymap_changed` event,
//! not a 1 s keymap clock. GPUI's test clock never moves here, so a clock could not bring it.
mod m10b_support;

use gpui::TestAppContext;
use m10b_support::*;
use serde_json::json;

#[gpui::test]
fn a_reload_reaches_the_window_by_its_event(cx: &mut TestAppContext) {
    let s = setup_with_config("leon.set(\"font.size\", 13)\n");
    let (ws, cx) = open(cx, &s.app);
    let size = |cx: &mut gpui::VisualTestContext| ws.read_with(cx, |ws, _| ws.font_size());
    assert!(shows(cx, |cx| size(cx) == 13.), "the config's size first");
    std::fs::write(s.dir.path().join("init.lua"), "leon.set(\"font.size\", 17)\n").unwrap();
    s.app.call("server.config_reload", json!({})).unwrap();
    let followed = shows(cx, |cx| size(cx) == 17.);
    s.app.stop();
    assert!(followed, "the reload's settings reached the window without its clock moving: {}", size(cx));
}
