//! Review proof (skeptic): a hover preview whose tab is closed under the pointer must not stay up
//! once the pointer leaves.

mod m10b_support;

use std::time::Duration;

use gpui::{Modifiers, TestAppContext};
use m10b_support::*;

#[gpui::test]
fn a_closed_tabs_preview_goes_away(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    act(&ws, cx, "client.tab_style.toggle");
    act(&ws, cx, "client.tab.new");
    act(&ws, cx, "client.tab.new");
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.windows()).is_some_and(|(w, _)| w.len() == 3)));
    let (wins, active) = ws.read_with(cx, |ws, _| ws.windows()).unwrap();
    let gone = wins[active].id.clone();

    let tab = format!("tab-{active}");
    let b = at(cx, &tab).unwrap_or_else(|| panic!("{tab} is drawn"));
    cx.simulate_mouse_move(center(b), None, Modifiers::none());
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(450));
    cx.run_until_parked();
    assert_eq!(ws.read_with(cx, |ws, _| ws.hover_preview()), Some(gone.clone()), "the preview shows first");

    // the hovered tab closes (its × clicked, or ⌘W on its only pane)
    act(&ws, cx, "pane.close");
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.windows()).is_some_and(|(w, _)| w.len() == 2)));

    // the pointer leaves the tabs
    let p = at(cx, "panes-area").expect("panes drawn");
    cx.simulate_mouse_move(center(p), None, Modifiers::none());
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(450));
    cx.run_until_parked();
    let shown = ws.read_with(cx, |ws, _| ws.hover_preview());
    assert_eq!(shown, None, "a closed tab's preview stays up after the pointer left");
    s.app.stop();
}

#[gpui::test]
fn a_titlebar_preview_goes_with_its_session(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    act(&ws, cx, "client.tab_style.toggle");
    let first = ws.read_with(cx, |ws, _| ws.session_id()).unwrap();
    let b = at(cx, "tab-0").expect("tab-0 is drawn");
    cx.simulate_mouse_move(center(b), None, Modifiers::none());
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(450));
    cx.run_until_parked();
    assert!(ws.read_with(cx, |ws, _| ws.hover_preview()).is_some(), "the preview shows first");
    let other = s.app.call("session.create", serde_json::json!({ "label": "ci" })).unwrap();
    let other = other["session_id"].as_str().unwrap().to_string();
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.session_count()) == 2));
    ws.update(cx, |ws, cx| ws.select(&other, None, cx));
    cx.run_until_parked();
    assert_ne!(ws.read_with(cx, |ws, _| ws.session_id()), Some(first));
    assert_eq!(ws.read_with(cx, |ws, _| ws.hover_preview()), None, "the other session's preview went with it");
    s.app.stop();
}
