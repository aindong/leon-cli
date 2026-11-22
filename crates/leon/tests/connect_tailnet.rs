//! The Connect popover's tailnet machines (.agent/plans/2026-11-22-sidebar-footer-connect.md,
//! criterion 5): from the local tailscaled's LocalAPI `status` (a fake here), only the owner's
//! other untagged machines that aren't hosts yet; listing asks for nothing but the status; one
//! chosen is saved as `tailnet://NAME` and joins the window.

// macOS only: drives the GPUI app through support/app.rs (the `app` feature).
#![cfg(all(feature = "app", target_os = "macos"))]

#[path = "support/app.rs"]
mod app;
mod support;

use app::*;
use gpui::TestAppContext;
use serde_json::json;
use support::localapi::{status, Fake, OWNER};

#[gpui::test]
fn the_owners_machines_from_the_localapi(cx: &mut TestAppContext) {
    let h = hosted();
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new().mode(0o700).create(h.r.local.state_dir()).unwrap();
    let fake = Fake::new();
    let mut st = status(OWNER, &["100.64.0.1"]);
    st["Self"]["DNSName"] = json!("mac.example-tailnet.ts.net.");
    let peer = |id: &str, user: u64, dns: &str, online: bool, tags: serde_json::Value| json!({ "ID": id, "UserID": user, "DNSName": dns, "Online": online, "Tags": tags });
    st["Peer"] = json!({
        "a": peer("n1", 7, "devbox.example-tailnet.ts.net.", true, json!(null)),
        "b": peer("n2", 7, "laptop.example-tailnet.ts.net.", false, json!(null)),
        "c": peer("n3", 7, "ci.example-tailnet.ts.net.", true, json!(["tag:ci"])),
        "d": peer("n4", 8, "theirs.example-tailnet.ts.net.", true, json!(null)),
        "e": peer("nself", 7, "mac.example-tailnet.ts.net.", true, json!(null)),
        "f": peer("n6", 7, "known.example-tailnet.ts.net.", true, json!(null)),
        "g": peer("n7", 7, "short.example-tailnet.ts.net.", true, json!(null)),
        "h": peer("n8", 7, "named.example-tailnet.ts.net.", true, json!(null)),
    });
    fake.set_status(st);
    std::env::set_var("LEON_TEST_TAILNET", fake.seam());
    let (ws, cx) = open(
        cx,
        &h,
        vec![
            host("known", "tailnet://known.example-tailnet.ts.net"),
            host("short", "ssh://unreachable-box"),
            host("box3", "tailnet://named"),
        ],
        None,
    );

    act(&ws, cx, "client.connect", None);
    let ids = |cx: &mut gpui::VisualTestContext| -> Vec<String> {
        ws.read_with(cx, |ws, cx| {
            ws.connect_menu().map(|p| p.read(cx).items().iter().map(|i| i.id.clone()).collect()).unwrap_or_default()
        })
    };
    assert!(
        shows(cx, |cx| ids(cx).iter().any(|i| i.starts_with("tailnet:"))),
        "the tailnet's rows come: {:?}",
        ids(cx)
    );
    let tailnet: Vec<String> = ids(cx).into_iter().filter(|i| i.starts_with("tailnet:")).collect();
    assert_eq!(
        tailnet,
        ["tailnet:devbox.example-tailnet.ts.net", "tailnet:laptop.example-tailnet.ts.net"],
        "only the owner's other untagged machines that aren't hosts (by name or label), online first"
    );
    let labels: Vec<String> = ws
        .read_with(cx, |ws, cx| ws.connect_menu().unwrap().read(cx).items().iter().map(|i| i.label.clone()).collect());
    assert!(labels.contains(&"On your tailnet".to_string()), "{labels:?}");
    let asked = fake.paths();
    assert!(!asked.is_empty() && asked.iter().all(|p| p.starts_with("/localapi/v0/status")), "read only: {asked:?}");

    let sel = "popover-item-tailnet:devbox.example-tailnet.ts.net";
    let b = at(cx, sel).expect("the row is drawn");
    cx.simulate_click(
        gpui::point(b.origin.x + b.size.width / 2., b.origin.y + b.size.height / 2.),
        gpui::Modifiers::none(),
    );
    cx.run_until_parked();
    let saved = leon_hosts::hosts::load().unwrap();
    assert_eq!(saved, vec![host("devbox", "tailnet://devbox.example-tailnet.ts.net")], "saved as a tailnet:// host");
    let shown = ws.read_with(cx, |ws, _| ws.host_rows().iter().any(|r| r.label == "devbox"));
    assert!(shown, "and it joins the window");
    h.local.stop();
}
