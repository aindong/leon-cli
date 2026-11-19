//! GPUI phase 5: a config reload reaches the window through the server's `keymap_changed` event,
//! not a 1 s keymap clock. GPUI's test clock never moves here, so a clock could not bring it.
mod m10b_support;

use gpui::TestAppContext;
use leon_lua::rewrite::write_setting;
use m10b_support::*;
use serde_json::json;

#[gpui::test]
fn a_reload_reaches_the_window_by_its_event(cx: &mut TestAppContext) {
    let s = setup_with_config("leon.set(\"translucency\", 0)\n");
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.set_reduce_transparency(Some(false)));
    let alpha = |cx: &mut gpui::VisualTestContext| ws.read_with(cx, |ws, _| ws.glass().chrome_alpha());
    assert!(shows(cx, |cx| alpha(cx) == 1.), "opaque at 0");
    write_setting(&s.dir.path().join("init.lua"), "translucency", &json!(0.3)).unwrap();
    s.app.call("server.config_reload", json!({})).unwrap();
    let followed = shows(cx, |cx| alpha(cx) < 1.);
    s.app.stop();
    assert!(followed, "the reload's settings reached the window without its clock moving");
}
