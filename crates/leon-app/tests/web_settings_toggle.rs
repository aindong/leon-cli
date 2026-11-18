//! Settings → Server → Tailscale: `serve_web` (docs/web.md) has a switch, which writes init.lua as
//! `leon config set` would.
mod m9_support;
mod settings_support;
use gpui::TestAppContext;
use serde_json::json;
use settings_support::*;

#[gpui::test]
fn the_web_switch_writes_serve_web(cx: &mut TestAppContext) {
    let (o, mut cx) = open_settings(cx, "");
    click(&mut cx, "section-server");
    click(&mut cx, "toggle-serve_web");
    let ok = saved(&mut cx, &o) && shows(&mut cx, |cx| value(cx, &o, "serve_web") == Some(json!(true)));
    let got = text(&o.config);
    o.s.app.stop();
    assert!(ok, "the window never showed serve_web on");
    assert_eq!(got, config_set_text("", "serve_web", &json!(true)));
}
