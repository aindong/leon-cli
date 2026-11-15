//! M22b impact: the running app picks up a settings change through its own keymap poll (no
//! `sync_keymap`): the generation moves, `server.settings` is fetched, and the theme follows.
mod m9_support;
use std::time::{Duration, Instant};

use gpui::TestAppContext;
use leon_app::theme::{DARK, LIGHT};
use m9_support::*;
use serde_json::json;

#[gpui::test]
fn the_poll_applies_a_settings_change(cx: &mut TestAppContext) {
    let s = setup_with_config("leon.set(\"theme\", \"leon-dark\")\n");
    let (ws, cx) = open(cx, &s.app);
    leon_lua::rewrite::write_setting(&s.dir.path().join("init.lua"), "theme", &json!("leon-light")).unwrap();
    s.app.call("server.config_reload", json!({})).unwrap();
    let end = Instant::now() + Duration::from_secs(10);
    // the poll runs on the app's timer; the keymap and settings replies come on threads of their own
    while ws.read_with(cx, |ws, _| ws.theme()) != LIGHT && Instant::now() < end {
        cx.executor().advance_clock(Duration::from_millis(1100));
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(20));
    }
    let got = ws.read_with(cx, |ws, _| ws.theme());
    s.app.stop();
    assert_ne!(got, DARK, "the poll never applied the change");
    assert_eq!(got, LIGHT);
}
