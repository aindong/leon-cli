//! The Settings window's Terminal section has a toggle for the link card, which writes init.lua as
//! `leon config set link_card false` does.
mod m9_support;
mod settings_support;

use gpui::TestAppContext;
use serde_json::json;
use settings_support::*;

const SRC: &str = "-- mine\nleon.set(\"link_card\", true)\n";

#[gpui::test]
fn the_toggle_turns_it_off(cx: &mut TestAppContext) {
    let (o, mut cx) = open_settings(cx, SRC);
    assert_eq!(value(&mut cx, &o, "link_card"), Some(json!(true)));
    click(&mut cx, "section-terminal");
    click(&mut cx, "toggle-link_card");
    let ok = saved(&mut cx, &o) && shows(&mut cx, |cx| value(cx, &o, "link_card") == Some(json!(false)));
    let got = text(&o.config);
    o.s.app.stop();
    assert!(ok, "the window never showed false");
    assert_eq!(got, config_set_text(SRC, "link_card", &json!(false)));
}
