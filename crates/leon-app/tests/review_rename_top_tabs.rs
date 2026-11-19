//! CodeRabbit on #94: a rename ends only when its field isn't drawn. In top tab style a tab's field
//! is in the titlebar: hiding the sidebar keeps it (and its text); switching to another session
//! hides it, and the keys go back to the pane.
mod focus_support;

use focus_support::*;
use gpui::TestAppContext;

fn rename_top_tab(ws: &gpui::Entity<leon_app::view::Workspace>, cx: &mut gpui::VisualTestContext) {
    act(ws, cx, "client.tab_style.toggle");
    let tab = center(at(cx, "tab-0").expect("the top tab"));
    double_click(cx, tab);
    assert_eq!(owner(ws, cx), Some("rename"), "the field has the focus");
}

#[gpui::test]
fn a_top_tab_rename_outlives_hiding_the_sidebar(cx: &mut TestAppContext) {
    let s = plain_setup();
    let (ws, cx) = open(cx, &s.app);
    settle(&focused(&s.app, &ws, cx));
    rename_top_tab(&ws, cx);
    cx.simulate_keystrokes("x");
    act(&ws, cx, "client.sidebar.toggle");
    assert!(!ws.read_with(cx, |ws, _| ws.sidebar_visible()), "the sidebar is hidden");
    let kept = owner(&ws, cx);
    cx.simulate_keystrokes("y enter");
    let named = shows(cx, |cx| ws.read_with(cx, |ws, _| ws.tab_titles()) == ["xy"]);
    s.app.stop();
    assert_eq!(kept, Some("rename"), "the field in the titlebar keeps the focus");
    assert!(named, "and its text: the tab is named xy");
}

#[gpui::test]
fn a_top_tab_rename_hidden_by_another_session_gives_the_keys_back(cx: &mut TestAppContext) {
    let s = plain_setup();
    let (ws, cx) = open(cx, &s.app);
    settle(&focused(&s.app, &ws, cx));
    rename_top_tab(&ws, cx);
    act(&ws, cx, "session.new");
    assert!(shows(cx, |_| sessions(&s) == 2), "a second session");
    settle(&focused(&s.app, &ws, cx));
    let r = back_on_pane(&s, &ws, cx, "other1");
    s.app.stop();
    r.unwrap();
}
