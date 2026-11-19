//! `leon.set("link_card", false)` turns the link card off: no card and no lookup on a rest or a
//! right-click. `server.settings` reports it, on by default.
mod link_card_support;

use gpui::{Modifiers, TestAppContext};
use link_card_support::*;
use serde_json::json;

#[gpui::test]
fn off_shows_no_card_and_asks_nothing(cx: &mut TestAppContext) {
    let s = setup_with_config("leon.set(\"link_card\", false)\n");
    assert_eq!(s.app.call("server.settings", json!({})).unwrap()["settings"]["link_card"], json!(false));
    let (ws, cx) = open(cx, &s.app);
    let l = links(&s, &ws, cx);
    rest(&ws, cx, l.web, Modifiers::none(), 600);
    right_click(cx, l.web);
    let (shown, asked) = (card(&ws, cx), asks(&ws, cx));
    s.app.stop();
    assert_eq!(shown, None, "no card");
    assert_eq!(asked, 0, "no lookup");
}

#[test]
fn on_by_default_and_a_known_key() {
    assert!(leon_lua::settings::KEYS.contains(&"link_card"), "leon config get/set take it");
    let s = setup();
    let got = s.app.call("server.settings", json!({})).unwrap();
    s.app.stop();
    assert_eq!(got["settings"]["link_card"], json!(true), "on by default: {got}");
}
