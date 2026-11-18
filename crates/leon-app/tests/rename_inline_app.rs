//! A double-click on a tab's or a session's name turns it into a field holding the name, all of it
//! selected: typing replaces it, Backspace edits, Enter (or a click elsewhere) sets it, Escape
//! cancels; meanwhile keys go to the field, not the pane.
mod close_support;

use close_support::*;
use gpui::{Modifiers, TestAppContext};
use serde_json::json;

fn renaming(ws: &gpui::Entity<leon_app::view::Workspace>, cx: &mut gpui::VisualTestContext) -> Option<String> {
    ws.read_with(cx, |ws, _| ws.renaming())
}

#[gpui::test]
fn a_sidebar_tab_row_and_session_label(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let sid = ws.read_with(cx, |ws, _| ws.session_id()).unwrap();
    let p = center(at(cx, "row-0-0").expect("the tab row"));
    double_click(cx, p);
    // the name as it is now: a shell can set its title just after it starts
    let name = ws.read_with(cx, |ws, _| ws.tab_titles())[0].clone();
    assert_eq!(renaming(&ws, cx).as_deref(), Some(name.as_str()), "the field holds the name");
    assert!(drawn(cx, "rename-field"), "and is drawn");
    cx.simulate_keystrokes("a p i x backspace");
    assert_eq!(renaming(&ws, cx).as_deref(), Some("api"), "typing replaced the name; backspace edits");
    let pane = ws.read_with(cx, |ws, _| ws.target_block()).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert!(!term(&s.app, &pane).text(true).contains("apix"), "the keys went to the field, not the pane");
    cx.simulate_keystrokes("enter");
    assert_eq!(renaming(&ws, cx), None);
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.tab_titles()) == ["api"]), "the tab is api");

    let p = center(at(cx, "row-0-0").expect("the tab row"));

    double_click(cx, p);
    cx.simulate_keystrokes("z escape");
    assert_eq!(renaming(&ws, cx), None, "Escape ends it");
    cx.run_until_parked();
    assert_eq!(ws.read_with(cx, |ws, _| ws.tab_titles()), ["api"], "and changes nothing");

    let p = center(at(cx, "session-0").expect("the session label"));

    double_click(cx, p);
    assert!(renaming(&ws, cx).is_some(), "the session's name too");
    assert!(drawn(cx, "row-0-0"), "and its tabs stay shown (the first click doesn't fold it)");
    cx.simulate_keystrokes("o p s");
    // a click elsewhere sets it
    let pane = format!("terminal-{}", ws.read_with(cx, |ws, _| ws.target_block()).unwrap());
    let p = center(at(cx, &pane).unwrap());
    cx.simulate_click(p, Modifiers::none());
    let named = shows(cx, |_| label(&s, &sid) == json!("ops"));
    s.app.stop();
    assert!(named, "the session is ops");
}

#[gpui::test]
fn a_titlebar_tab(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    act(&ws, cx, "client.tab_style.toggle");
    let p = center(at(cx, "tab-0").expect("the titlebar tab"));
    double_click(cx, p);
    assert!(renaming(&ws, cx).is_some(), "the field opens");
    cx.simulate_keystrokes("d b enter");
    let named = shows(cx, |cx| ws.read_with(cx, |ws, _| ws.tab_titles()) == ["db"]);
    s.app.stop();
    assert!(named, "the tab is db");
}
