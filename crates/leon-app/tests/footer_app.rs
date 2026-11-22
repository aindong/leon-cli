//! The sidebar's footer (.agent/plans/2026-11-22-sidebar-footer-connect.md, criteria 1–3, 6):
//! a row pinned under the session list with Connect on the left and Settings on the right; the
//! Connect popover from the footer, the palette and ⌘⇧K, by keyboard; Add a host….
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
fn the_footer_stays_at_the_bottom_and_hides_with_the_sidebar(cx: &mut TestAppContext) {
    let s = setup_with_config("");
    for _ in 0..40 {
        s.app.call("session.create", json!({})).unwrap();
    }
    let (ws, cx) = open(cx, &s.app);
    assert!(shows(cx, |cx| drawn(cx, "session-39")), "the long list is drawn");
    let footer = at(cx, "sidebar-footer").expect("the footer is drawn");
    let window = cx.update(|window, _| window.viewport_size());
    assert_eq!(footer.origin.y + footer.size.height, window.height, "pinned to the window's bottom");
    assert_eq!(f32::from(footer.size.height), 30., "a row high");
    let list = at(cx, "sidebar-list").expect("the list");
    assert!(list.origin.y + list.size.height <= footer.origin.y, "the list ends above it: {list:?} {footer:?}");
    let (connect, gear) = (at(cx, "footer-connect").unwrap(), at(cx, "footer-settings").unwrap());
    assert!(connect.origin.x < gear.origin.x, "Connect on the left, Settings on the right");
    assert!(footer.contains(&center(connect)) && footer.contains(&center(gear)));
    act(&ws, cx, "client.sidebar.toggle");
    let hidden = !drawn(cx, "sidebar-footer") && !drawn(cx, "footer-settings");
    s.app.stop();
    assert!(hidden, "it hides with the sidebar");
}

#[gpui::test]
fn the_gear_opens_settings(cx: &mut TestAppContext) {
    let s = setup_with_config("");
    let (ws, cx) = open(cx, &s.app);
    assert!(!ws.read_with(cx, |ws, _| ws.settings_open()));
    click(cx, "footer-settings");
    let open = ws.read_with(cx, |ws, _| ws.settings_open());
    s.app.stop();
    assert!(open, "the gear opens the Settings window");
}

#[gpui::test]
fn connect_opens_from_the_footer_the_palette_and_its_key(cx: &mut TestAppContext) {
    let s = setup_with_config("");
    let (ws, cx) = open(cx, &s.app);
    click(cx, "footer-connect");
    assert!(ws.read_with(cx, |ws, _| ws.connect_menu().is_some()), "the footer's Connect opens it");
    let all = labels(&ws, cx);
    assert!(all.iter().any(|l| l == "Add a host…"), "{all:?}");
    cx.simulate_keystrokes("escape");
    assert!(ws.read_with(cx, |ws, _| ws.connect_menu().is_none()), "Escape closes it");
    let back = cx.update(|window, cx| ws.read(cx).focus_owner(window, cx));
    assert_eq!(back, Some("pane"), "the focus goes back to the pane");

    let rows = ws.read_with(cx, |ws, _| ws.palette_rows_for("connect to a machine"));
    let row = rows.iter().find(|(t, _)| t == "Connect to a Machine…").cloned();
    assert_eq!(row.and_then(|(_, k)| k).as_deref(), Some("⌘⇧K"), "the palette offers it, with its key: {rows:?}");
    act(&ws, cx, "client.connect");
    assert!(ws.read_with(cx, |ws, _| ws.connect_menu().is_some()), "the action opens it");
    cx.simulate_keystrokes("escape");

    cx.simulate_keystrokes("cmd-shift-k");
    let open = ws.read_with(cx, |ws, _| ws.connect_menu().is_some());
    s.app.stop();
    assert!(open, "⌘⇧K opens it");
}

#[gpui::test]
fn add_a_host_by_keyboard_opens_the_add_host_flow(cx: &mut TestAppContext) {
    let s = setup_with_config("");
    let (ws, cx) = open(cx, &s.app);
    act(&ws, cx, "client.connect");
    let all = labels(&ws, cx);
    let add = all.iter().position(|l| l == "Add a host…").expect("Add a host… is listed");
    let first = ws.read_with(cx, |ws, cx| ws.connect_menu().unwrap().read(cx).highlighted());
    let first = first.expect("one is highlighted");
    for _ in first..add {
        cx.simulate_keystrokes("down");
    }
    cx.simulate_keystrokes("enter");
    let asking = ws.read_with(cx, |ws, _| ws.palette_open() && ws.connect_menu().is_none());
    let rows = ws.read_with(cx, |ws, _| ws.palette_rows());
    s.app.stop();
    assert!(asking, "Add a host… opens the Add Host prompt: {rows:?}");
    assert!(rows.iter().any(|(t, k)| (t.clone() + k.as_deref().unwrap_or("")).contains("label")), "{rows:?}");
}
