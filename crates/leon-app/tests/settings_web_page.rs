//! Settings → Server → Tailscale reads as two things: Tailscale access (its switch, what it says,
//! Port and Tags under a closed Advanced), then the Web page (its switch; the offer to turn on
//! Tailscale access too; while served, the address with Copy, a QR code and Other addresses; its
//! HTTPS files under a closed "HTTPS (optional)"). A seam points the server at a LocalAPI that
//! isn't there, so no test reaches this machine's Tailscale; the served states are shown with
//! `show_tailnet`, as `server.status` would report them.
mod m9_support;
mod settings_support;
use gpui::{TestAppContext, VisualTestContext};
use leon_app::settings_window::{web_state, WebState};
use serde_json::{json, Value};
use settings_support::*;

const DNS: &str = "http://devbox.example-tailnet.ts.net:7297/";
const IP: &str = "http://100.101.12.4:7297/";

/// Every test here sets the same seam before its server starts.
fn nowhere() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        std::env::set_var("LEON_TEST_TAILNET", json!({ "localapi": "/nonexistent/leon-test.sock" }).to_string())
    });
}

fn open(cx: &mut TestAppContext, src: &str) -> (Opened, VisualTestContext) {
    nowhere();
    let (o, mut vcx) = open_settings(cx, src);
    click(&mut vcx, "section-server");
    (o, vcx)
}

fn show(cx: &mut VisualTestContext, o: &Opened, tailnet: Value) {
    o.sw.update(cx, |sw, _, cx| sw.show_tailnet(tailnet, cx)).unwrap();
    cx.run_until_parked();
}

fn state(cx: &VisualTestContext, o: &Opened) -> WebState {
    o.sw.read_with(cx, |sw, _| sw.web()).unwrap()
}

fn top(cx: &mut VisualTestContext, sel: &'static str) -> f32 {
    cx.run_until_parked();
    f32::from(cx.debug_bounds(sel).unwrap_or_else(|| panic!("nothing drawn as {sel}")).origin.y)
}

fn serving(web: &[&str], error: Option<&str>) -> Value {
    json!({ "state": "serving", "port": 7297, "addresses": ["100.101.12.4:7297"], "login": "demo@example.com",
            "web": web, "web_error": error })
}

#[gpui::test]
fn access_comes_first_with_port_and_tags_under_advanced(cx: &mut TestAppContext) {
    let (o, mut cx) = open(cx, "");
    let closed = !drawn(&mut cx, "port-field") && !drawn(&mut cx, "tag-field");
    let order = [
        top(&mut cx, "toggle-serve_tailscale"),
        top(&mut cx, "tailscale-advanced"),
        top(&mut cx, "web-page-heading"),
        top(&mut cx, "toggle-serve_web"),
    ];
    click(&mut cx, "tailscale-advanced");
    let open = drawn(&mut cx, "port-field") && drawn(&mut cx, "tag-field");
    o.s.app.stop();
    assert!(closed, "Port and Tags are closed at first");
    assert!(order.windows(2).all(|w| w[0] < w[1]), "access, Advanced, then the Web page: {order:?}");
    assert!(open, "Advanced opens Port and Tags");
}

#[gpui::test]
fn the_web_page_asks_before_turning_on_tailscale_access(cx: &mut TestAppContext) {
    let src = "leon.set(\"serve_web\", true)\n";
    let (o, mut cx) = open(cx, src);
    let before = (state(&cx, &o), text(&o.config));
    click(&mut cx, "web-turn-on-tailscale");
    let ok = saved(&mut cx, &o) && shows(&mut cx, |cx| value(cx, &o, "serve_tailscale") == Some(json!(true)));
    let got = text(&o.config);
    o.s.app.stop();
    assert_eq!(before, (WebState::NeedsTailscale, src.to_string()), "it says so and writes nothing by itself");
    assert!(ok, "the button turned serve_tailscale on");
    assert_eq!(got, config_set_text(src, "serve_tailscale", &json!(true)));
}

#[gpui::test]
fn served_it_shows_the_address_copy_and_a_qr_code(cx: &mut TestAppContext) {
    let (o, mut cx) = open(cx, "leon.set(\"serve_tailscale\", true)\nleon.set(\"serve_web\", true)\n");
    show(&mut cx, &o, serving(&[DNS, IP], None));
    let others_closed = !drawn(&mut cx, "web-other-url-0");
    let parts = ["web-url", "web-copy", "web-qr", "web-qr-note"].map(|s| (s, drawn(&mut cx, s)));
    let qr = cx.debug_bounds("web-qr").unwrap();
    click(&mut cx, "web-copy");
    let copied = cx.read_from_clipboard().and_then(|c| c.text());
    click(&mut cx, "web-others");
    let others = drawn(&mut cx, "web-other-url-0");
    let (st, url, note) = o.sw.read_with(&cx, |sw, _| (sw.web(), sw.web_url(), sw.web_note())).unwrap();
    o.s.app.stop();
    assert_eq!(st, WebState::Serving { urls: vec![DNS.into(), IP.into()], error: None });
    assert!(parts.iter().all(|(_, d)| *d), "drawn: {parts:?}");
    assert_eq!(url.as_deref(), Some(DNS), "the MagicDNS address first");
    assert_eq!(copied.as_deref(), Some(DNS), "Copy puts it on the clipboard");
    let side = (f32::from(qr.size.width), f32::from(qr.size.height));
    assert!(side.0 == side.1 && (150.0..=170.0).contains(&side.0), "about 160 pt square: {side:?}");
    assert!(others_closed && others, "the IP address only under Other addresses");
    assert!(note.contains("this Mac's own browser is refused"), "{note}");
}

#[gpui::test]
fn a_web_error_and_a_stopped_server_are_said(cx: &mut TestAppContext) {
    let (o, mut cx) = open(cx, "leon.set(\"serve_tailscale\", true)\nleon.set(\"serve_web\", true)\n");
    let why = "the certificate in /x/web.crt is no longer valid (expired). Make them with: tailscale cert …";
    show(&mut cx, &o, serving(&[], Some(why)));
    let err = (state(&cx, &o), drawn(&mut cx, "web-error"));
    show(&mut cx, &o, json!({ "state": "stopped", "reason": "Tailscale isn't running", "web": [] }));
    let stopped = (state(&cx, &o), drawn(&mut cx, "web-not-serving"));
    o.s.app.stop();
    assert_eq!(err, (WebState::Serving { urls: vec![], error: Some(why.into()) }, true));
    assert_eq!(stopped, (WebState::NotServing("Tailscale isn't running".into()), true));
}

#[test]
fn the_states() {
    let t = |v: Value| Some(v);
    assert_eq!(web_state(false, true, t(serving(&[DNS], None)).as_ref()), WebState::Off);
    assert_eq!(web_state(true, false, None), WebState::NeedsTailscale);
    assert_eq!(web_state(true, true, None), WebState::Waiting, "nothing reported yet: nothing");
    assert_eq!(web_state(true, true, t(json!({ "state": "off" })).as_ref()), WebState::Waiting);
    assert_eq!(web_state(true, true, t(serving(&[], None)).as_ref()), WebState::Waiting, "no address yet");
    let stopped = json!({ "state": "stopped", "reason": "no LocalAPI" });
    assert_eq!(web_state(true, true, t(stopped).as_ref()), WebState::NotServing("no LocalAPI".into()));
}

#[gpui::test]
fn the_https_files_are_under_their_own_disclosure(cx: &mut TestAppContext) {
    let (o, mut cx) = open(cx, "");
    let closed = !drawn(&mut cx, "web-cert-field") && !drawn(&mut cx, "web-key-field");
    click(&mut cx, "web-https");
    let open =
        drawn(&mut cx, "web-cert-field") && drawn(&mut cx, "web-key-field") && drawn(&mut cx, "web-cert-command");
    let cmd = o.sw.read_with(&cx, |sw, _| sw.cert_hint()).unwrap();
    o.s.app.stop();
    assert!(closed && open, "closed at first, open on a click");
    assert!(cmd.contains(" cert --cert-file ") && cmd.contains("--key-file "), "{cmd}");
}

/// While "HTTPS (optional)" is open, the command follows the files set: it is made from the
/// settings each time, and only the program it names is looked up once.
#[gpui::test]
fn the_cert_command_follows_the_files_while_open(cx: &mut TestAppContext) {
    let (o, mut cx) = open(cx, "");
    click(&mut cx, "web-https");
    let before = o.sw.read_with(&cx, |sw, _| sw.cert_hint()).unwrap();
    o.sw.update(&mut cx, |sw, _, cx| {
        sw.choose("serve_web_cert", json!("/certs/b.crt"), cx);
        sw.choose("serve_web_key", json!("/certs/b.key"), cx);
    })
    .unwrap();
    let saved = saved(&mut cx, &o);
    let after = o.sw.read_with(&cx, |sw, _| sw.cert_hint()).unwrap();
    o.s.app.stop();
    assert!(saved, "the files were written");
    assert!(!before.contains("/certs/b.crt"), "{before}");
    assert!(after.contains("--cert-file '/certs/b.crt' --key-file '/certs/b.key'"), "{after}");
}

/// The machine's name in the command is the MagicDNS name the server said, and only a DNS name:
/// anything else (a shell word, an address) leaves the placeholder.
#[gpui::test]
fn the_cert_command_names_the_machine_only_by_a_dns_name(cx: &mut TestAppContext) {
    let (o, mut cx) = open(cx, "leon.set(\"serve_tailscale\", true)\nleon.set(\"serve_web\", true)\n");
    let hint = |cx: &mut VisualTestContext, url: &str| {
        show(cx, &o, serving(&[url], None));
        o.sw.read_with(cx, |sw, _| sw.cert_hint()).unwrap()
    };
    let good = hint(&mut cx, DNS);
    let bad = hint(&mut cx, "http://x;touch$IFS/tmp/p:7297/");
    let ip = hint(&mut cx, IP);
    o.s.app.stop();
    assert!(good.ends_with(" devbox.example-tailnet.ts.net"), "{good}");
    assert!(bad.ends_with(" MACHINE.TAILNET.ts.net") && !bad.contains("touch"), "{bad}");
    assert!(ip.ends_with(" MACHINE.TAILNET.ts.net"), "{ip}");
}
