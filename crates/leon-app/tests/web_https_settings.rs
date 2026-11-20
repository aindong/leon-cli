//! Settings → Server → Tailscale: `serve_web_cert` and `serve_web_key` (docs/web.md) have a field
//! each, which writes init.lua as `leon config set` would and refuses a path the setting refuses.
mod m9_support;
mod settings_support;
use gpui::{TestAppContext, VisualTestContext};
use serde_json::json;
use settings_support::*;

fn enter(cx: &mut VisualTestContext, field: &'static str, input: &str) {
    click(cx, field);
    cx.simulate_input(input);
    cx.simulate_keystrokes("enter");
}

#[gpui::test]
fn the_https_fields_write_the_paths(cx: &mut TestAppContext) {
    let (o, mut cx) = open_settings(cx, "");
    click(&mut cx, "section-server");
    enter(&mut cx, "web-cert-field", "relative.crt");
    let refused = text(&o.config);
    cx.simulate_keystrokes("escape");
    enter(&mut cx, "web-cert-field", "/certs/box.crt");
    let ok =
        saved(&mut cx, &o) && shows(&mut cx, |cx| value(cx, &o, "serve_web_cert") == Some(json!("/certs/box.crt")));
    let got = text(&o.config);
    o.s.app.stop();
    assert_eq!(refused, "", "a relative path is refused, nothing written");
    assert!(ok, "the window never showed serve_web_cert set");
    assert_eq!(got, config_set_text("", "serve_web_cert", &json!("/certs/box.crt")));
}
