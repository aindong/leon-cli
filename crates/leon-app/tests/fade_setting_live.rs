//! A running app follows `fade_inactive_panes`: 0 draws every pane as before the fade existed,
//! and a new strength shows without a restart.

mod m10b_support;

use gpui::{Entity, TestAppContext, VisualTestContext};
use leon_app::view::Workspace;
use leon_lua::rewrite::write_setting;
use m10b_support::*;
use serde_json::json;

/// Longer than the app waits between two looks at the config: a wait moves the app's timer on
/// by this, and never reads the settings or asks for a redraw itself.
const POLL: std::time::Duration = std::time::Duration::from_millis(1100);

/// What `leon config set fade_inactive_panes N` does: rewrite init.lua, reload the server. The
/// app hears of it through its own poll.
fn config_set(s: &Setup, n: f64) {
    write_setting(&s.dir.path().join("init.lua"), "fade_inactive_panes", &json!(n)).unwrap();
    s.app.call("server.config_reload", json!({})).unwrap();
}

/// The two panes' fades once they are at rest at `want` (else what they were).
fn rests_at(ws: &Entity<Workspace>, cx: &mut VisualTestContext, ids: [&str; 2], want: [f32; 2]) -> [f32; 2] {
    let now = |ws: &Workspace| ids.map(|id| ws.pane_fade(id));
    shows(cx, |cx| {
        cx.executor().advance_clock(POLL);
        cx.run_until_parked();
        ws.read_with(cx, |ws, _| !ws.fading() && now(ws) == want)
    });
    ws.read_with(cx, |ws, _| now(ws))
}

#[gpui::test]
fn the_app_follows_the_setting(cx: &mut TestAppContext) {
    let s = setup_with_config("leon.set(\"fade_inactive_panes\", 0)\n");
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, cx| ws.sync_keymap(cx));
    let first = ws.read_with(cx, |ws, _| ws.target_block()).expect("a pane");
    act(&ws, cx, "pane.split.right");
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.target_block().is_some_and(|t| t != first))), "no split");
    let second = ws.read_with(cx, |ws, _| ws.target_block()).unwrap();
    let ids = [first.as_str(), second.as_str()];

    let off = rests_at(&ws, cx, ids, [0., 0.]);
    let (unfocused, window) = ws.read_with(cx, |ws, _| (ws.pane_theme(&first), ws.theme()));
    config_set(&s, 0.5);
    let half = rests_at(&ws, cx, ids, [0.5, 0.]);
    config_set(&s, 0.2);
    let fifth = rests_at(&ws, cx, ids, [0.2, 0.]);
    config_set(&s, 0.);
    let off_again = rests_at(&ws, cx, ids, [0., 0.]);
    s.app.stop();
    assert_eq!(off, [0., 0.], "0 turns the fade off");
    assert_eq!(unfocused, window, "off: the unfocused pane is drawn with the window's theme, as before the fade");
    assert_eq!(half, [0.5, 0.], "a new strength shows without a restart");
    assert_eq!(fifth, [0.2, 0.]);
    assert_eq!(off_again, [0., 0.], "and it turns off again");
}
