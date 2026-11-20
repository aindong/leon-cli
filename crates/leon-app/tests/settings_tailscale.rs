//! Settings → Server → Tailscale, under Advanced: `serve_tailscale_port` has a field and `serve_tailscale_tags` a
//! tag list. Each writes init.lua as `leon config set` would, refuses what the setting refuses
//! (saying why, writing nothing), and is read-only when init.lua sets it by code.
mod m9_support;
mod settings_support;
use gpui::{TestAppContext, VisualTestContext};
use serde_json::json;
use settings_support::*;

/// Type `input` into the field drawn as `field` and press Enter.
fn enter(cx: &mut VisualTestContext, field: &'static str, input: &str) {
    click(cx, field);
    cx.simulate_input(input);
    cx.simulate_keystrokes("enter");
}

/// Why the port and the tag were last refused. (Not `drawn`: GPUI's test frames keep every
/// selector ever drawn, so an error once shown would read as shown for ever.)
fn errors(cx: &VisualTestContext, o: &Opened) -> [Option<String>; 2] {
    o.sw.read_with(cx, |sw, _| sw.tailscale_errors()).unwrap()
}

#[gpui::test]
fn the_port_field_writes_a_port_and_refuses_anything_else(cx: &mut TestAppContext) {
    let (o, mut cx) = open_settings(cx, "");
    click(&mut cx, "section-server");
    click(&mut cx, "tailscale-advanced"); // Port and Tags sit under Advanced
    let mut refused = vec![];
    for bad in ["0", "70000", "abc", "80.5"] {
        enter(&mut cx, "port-field", bad);
        assert!(saved(&mut cx, &o));
        refused.push((bad, errors(&cx, &o)[0].clone(), drawn(&mut cx, "port-error"), text(&o.config)));
        cx.simulate_keystrokes("escape");
    }
    enter(&mut cx, "port-field", "8022");
    let ok = saved(&mut cx, &o) && shows(&mut cx, |cx| value(cx, &o, "serve_tailscale_port") == Some(json!(8022)));
    let (got, error) = (text(&o.config), errors(&cx, &o)[0].clone());
    o.s.app.stop();
    for (bad, why, shown, written) in refused {
        assert!(why.is_some_and(|w| w.contains("1 to 65535")), "{bad}: the setting's own error");
        assert!(shown, "{bad}: the error is drawn under the field");
        assert_eq!(written, "", "{bad}: nothing written");
    }
    assert!(ok, "the window never showed 8022");
    assert_eq!(error, None, "no error left once a port is written");
    assert_eq!(got, config_set_text("", "serve_tailscale_port", &json!(8022)));
}

#[gpui::test]
fn the_tag_list_adds_and_removes_tags_and_refuses_a_bad_one(cx: &mut TestAppContext) {
    let src = "leon.set(\"serve_tailscale_tags\", { \"tag:a\", \"tag:b\" })\n";
    let (o, mut cx) = open_settings(cx, src);
    click(&mut cx, "section-server");
    click(&mut cx, "tailscale-advanced"); // Port and Tags sit under Advanced
    enter(&mut cx, "tag-field", "ci");
    assert!(saved(&mut cx, &o));
    let (bad_why, bad_shown, bad_written) = (errors(&cx, &o)[1].clone(), drawn(&mut cx, "tag-error"), text(&o.config));
    cx.simulate_keystrokes("escape");
    enter(&mut cx, "tag-field", "tag:a");
    assert!(saved(&mut cx, &o));
    let (dup_why, dup_written) = (errors(&cx, &o)[1].clone(), text(&o.config));

    click(&mut cx, "tag-remove-0");
    enter(&mut cx, "tag-field", "tag:ci");
    let after = json!(["tag:b", "tag:ci"]);
    let ok = saved(&mut cx, &o) && shows(&mut cx, |cx| value(cx, &o, "serve_tailscale_tags") == Some(after.clone()));
    let got = text(&o.config);
    o.s.app.stop();
    assert!(bad_why.is_some_and(|w| w.contains("tag:ci")), "a tag without tag: gets the setting's error");
    assert!(bad_shown, "drawn under the field");
    assert_eq!(bad_written, src, "and writes nothing");
    assert_eq!((dup_why, dup_written.as_str()), (None, src), "a tag already there writes nothing");
    assert!(ok, "the window never showed {after}");
    assert_eq!(got, config_set_text(src, "serve_tailscale_tags", &after));
}

#[gpui::test]
fn a_seventeenth_tag_is_refused(cx: &mut TestAppContext) {
    let tags: Vec<String> = (0..leon_lua::settings::MOST_TAGS).map(|i| format!("\"tag:t{i}\"")).collect();
    let src = format!("leon.set(\"serve_tailscale_tags\", {{ {} }})\n", tags.join(", "));
    let (o, mut cx) = open_settings(cx, &src);
    click(&mut cx, "section-server");
    click(&mut cx, "tailscale-advanced"); // Port and Tags sit under Advanced
    enter(&mut cx, "tag-field", "tag:more");
    assert!(saved(&mut cx, &o));
    let (why, shown, got) = (errors(&cx, &o)[1].clone(), drawn(&mut cx, "tag-error"), text(&o.config));
    o.s.app.stop();
    assert!(why.is_some_and(|w| w.contains("at most 16")), "the list is full: it says so");
    assert!(shown, "drawn under the field");
    assert_eq!(got, src, "nothing written");
}

#[gpui::test]
fn port_and_tags_set_by_code_are_read_only(cx: &mut TestAppContext) {
    let src = "local p, ts = 8000, { \"tag:a\" }\nleon.set(\"serve_tailscale_port\", p)\nleon.set(\"serve_tailscale_tags\", ts)\n";
    let (o, mut cx) = open_settings(cx, src);
    click(&mut cx, "section-server");
    click(&mut cx, "tailscale-advanced"); // Port and Tags sit under Advanced
    let locked = drawn(&mut cx, "locked-serve_tailscale_port") && drawn(&mut cx, "locked-serve_tailscale_tags");
    let fields = drawn(&mut cx, "port-field") || drawn(&mut cx, "tag-field");
    click(&mut cx, "tag-remove-0");
    assert!(saved(&mut cx, &o));
    let got = text(&o.config);
    o.s.app.stop();
    assert!(locked, "each says why it can't be changed");
    assert!(!fields, "no field to type into");
    assert_eq!(got, src, "nothing written");
}
