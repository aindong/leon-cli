//! A right-click on a sidebar tab row or a titlebar tab opens Rename Tab and Close Tab; on a
//! session, New Tab, Rename Session and Close Session; each does what its action does.
mod close_support;

use close_support::*;
use gpui::TestAppContext;

fn items(ws: &gpui::Entity<leon_app::view::Workspace>, cx: &mut gpui::VisualTestContext) -> Vec<String> {
    ws.read_with(cx, |ws, cx| ws.row_menu().map(|p| p.read(cx).items().iter().map(|i| i.label.clone()).collect()))
        .unwrap_or_default()
}

#[gpui::test]
fn the_sidebar_menus(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let sid = ws.read_with(cx, |ws, _| ws.session_id()).unwrap();
    let p = center(at(cx, "session-0").expect("the session label"));
    right_click(cx, p);
    assert_eq!(items(&ws, cx), ["New Tab", "Rename Session…", "Close Session"]);
    cx.simulate_keystrokes("enter"); // New Tab, highlighted first
    assert!(shows(cx, |_| tabs(&s, &sid) == 2), "New Tab made one");
    let p = center(at(cx, "row-0-1").expect("the new tab's row"));
    right_click(cx, p);
    assert_eq!(items(&ws, cx), ["Rename Tab…", "Close Tab"]);
    cx.simulate_keystrokes("down enter"); // Close Tab: a shell, no question
    assert!(shows(cx, |_| tabs(&s, &sid) == 1), "Close Tab closed it");
    let p = center(at(cx, "row-0-0").expect("the tab row"));
    right_click(cx, p);
    cx.simulate_keystrokes("enter"); // Rename Tab…: the inline field
    let renaming = ws.read_with(cx, |ws, _| ws.renaming());
    s.app.stop();
    assert!(renaming.is_some(), "Rename starts the inline field");
}

#[gpui::test]
fn the_titlebar_tab_menu(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    act(&ws, cx, "client.tab_style.toggle");
    let p = center(at(cx, "tab-0").expect("the titlebar tab"));
    right_click(cx, p);
    assert_eq!(items(&ws, cx), ["Rename Tab…", "Close Tab"]);
    cx.simulate_keystrokes("escape");
    let open = ws.read_with(cx, |ws, _| ws.row_menu().is_some());
    s.app.stop();
    assert!(!open, "Escape closes it");
}
