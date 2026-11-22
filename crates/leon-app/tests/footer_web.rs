//! The Connect popover's web row (.agent/plans/2026-11-22-sidebar-footer-connect.md, criterion 7):
//! "Turn on in Settings…" while the page isn't served, else "Open on another device" with its
//! address and QR code.
mod m10b_support;

use gpui::{Entity, TestAppContext, VisualTestContext};
use leon_app::view::Workspace;
use m10b_support::*;
use serde_json::json;

fn items(ws: &Entity<Workspace>, cx: &mut VisualTestContext) -> Vec<(String, Option<String>)> {
    let menu = ws.read_with(cx, |ws, cx| {
        ws.connect_menu().map(|p| p.read(cx).items().iter().map(|i| (i.label.clone(), i.hint.clone())).collect())
    });
    menu.unwrap_or_default()
}

fn labels(ws: &Entity<Workspace>, cx: &mut VisualTestContext) -> Vec<String> {
    items(ws, cx).into_iter().map(|(l, _)| l).collect()
}

fn click(cx: &mut VisualTestContext, sel: &str) {
    let p = center(at(cx, sel).unwrap_or_else(|| panic!("{sel} is drawn")));
    cx.simulate_click(p, gpui::Modifiers::none());
    cx.run_until_parked();
}

#[gpui::test]
fn the_web_row_in_both_states(cx: &mut TestAppContext) {
    let s = setup_with_config("");
    let (ws, cx) = open(cx, &s.app);
    act(&ws, cx, "client.connect");
    assert!(labels(&ws, cx).iter().any(|l| l == "Turn on in Settings…"), "not served: {:?}", items(&ws, cx));
    cx.simulate_keystrokes("escape");

    let url = "https://mac.example-tailnet.ts.net:7297/";
    let tailnet = json!({ "state": "serving", "web": [url], "addresses": ["100.64.0.1"] });
    cx.update(|_, cx| ws.update(cx, |ws, cx| ws.show_web_status(Some(tailnet), cx)));
    act(&ws, cx, "client.connect");
    let row = items(&ws, cx).into_iter().find(|(l, _)| l == "Open on another device");
    assert!(row.as_ref().is_some_and(|(_, h)| h.as_deref() == Some(url)), "served: {:?}", items(&ws, cx));
    let all = labels(&ws, cx);
    let i = all.iter().position(|l| l == "Open on another device").unwrap();
    click(
        cx,
        &format!(
            "popover-item-{}",
            ws.read_with(cx, |ws, cx| ws.connect_menu().unwrap().read(cx).items()[i].id.clone())
        ),
    );
    let card = ws.read_with(cx, |ws, cx| ws.connect_menu().map(|p| (p.read(cx).lines().to_vec(), p.read(cx).has_qr())));
    let qr = drawn(cx, "popover-qr");
    s.app.stop();
    let (lines, has_qr) = card.expect("the web card is open");
    assert!(lines.iter().any(|l| l == url), "it shows the URL: {lines:?}");
    assert!(has_qr && qr, "and its QR code");
}
