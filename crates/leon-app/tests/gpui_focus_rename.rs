//! GPUI phase 6 (ADR R9): the inline rename field takes the focus and edits the name; Enter (it
//! renames) and Escape (it doesn't) give the focus back to the pane, and typed text reaches it.
mod focus_support;

use focus_support::*;
use gpui::TestAppContext;

#[gpui::test]
fn enter_and_escape_give_the_pane_its_keys_back(cx: &mut TestAppContext) {
    let s = plain_setup();
    let (ws, cx) = open(cx, &s.app);
    settle(&focused(&s.app, &ws, cx));
    let renaming = |cx: &mut gpui::VisualTestContext| ws.read_with(cx, |ws, _| ws.renaming());

    let row = center(at(cx, "row-0-0").expect("the tab row"));
    double_click(cx, row);
    assert!(renaming(cx).is_some(), "the field opened");
    assert_eq!(owner(&ws, cx), Some("rename"), "the field has the focus");
    cx.simulate_keystrokes("a p i");
    assert_eq!(renaming(cx).as_deref(), Some("api"), "typing edits the name");
    cx.simulate_keystrokes("enter");
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.tab_titles()) == ["api"]), "Enter renamed it");
    back_on_pane(&s, &ws, cx, "enter1").unwrap();

    let row = center(at(cx, "row-0-0").expect("the tab row"));
    double_click(cx, row);
    assert_eq!(owner(&ws, cx), Some("rename"), "the field has the focus again");
    cx.simulate_keystrokes("x escape");
    assert!(renaming(cx).is_none(), "Escape ended it");
    back_on_pane(&s, &ws, cx, "esc2").unwrap();

    // its tab goes away under it (closed from the CLI): the pane shown takes the keys
    let sid = ws.read_with(cx, |ws, _| ws.session_id()).unwrap();
    act(&ws, cx, "client.tab.new");
    assert!(shows(cx, |_| tabs(&s, &sid) == 2), "a second tab");
    settle(&focused(&s.app, &ws, cx));
    let row = center(at(cx, "row-0-1").expect("the second tab's row"));
    double_click(cx, row);
    assert_eq!(owner(&ws, cx), Some("rename"), "the field has the focus");
    let w = ws.read_with(cx, |ws, _| ws.windows()).unwrap().0[1].id.clone();
    s.app.call("session.close_window", serde_json::json!({ "session_id": sid, "window_id": w })).unwrap();
    assert!(shows(cx, |_| tabs(&s, &sid) == 1), "the tab is gone");
    assert!(shows(cx, |cx| renaming(cx).is_none()), "and the field with it");
    settle(&focused(&s.app, &ws, cx));
    let r = back_on_pane(&s, &ws, cx, "gone3");
    s.app.stop();
    r.unwrap();
}
