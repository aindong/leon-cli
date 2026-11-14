//! M10b criterion 1: tabs in one place. Sidebar style lists tabs as sidebar rows with no top tab
//! bar; top style puts them in the titlebar row and the sidebar lists sessions only; never both.

mod m10b_support;

use gpui::TestAppContext;
use leon_app::chrome::{places, SidebarList, TabStyle};
use m10b_support::*;

#[test]
fn the_layout_model_never_draws_tabs_twice() {
    for style in [TabStyle::Sidebar, TabStyle::Top] {
        for shown in [true, false] {
            let p = places(style, shown);
            let in_sidebar = p.sidebar == Some(SidebarList::SessionsAndTabs);
            assert!(!(in_sidebar && p.top_tabs), "{style:?}, sidebar shown {shown}: tabs in two places");
            assert_eq!(p.sidebar.is_some(), shown, "{style:?}: the sidebar shows only when shown");
        }
    }
    assert_eq!(TabStyle::default(), TabStyle::Sidebar);
    let side = places(TabStyle::Sidebar, true);
    assert_eq!((side.sidebar, side.top_tabs), (Some(SidebarList::SessionsAndTabs), false));
    let top = places(TabStyle::Top, true);
    assert_eq!((top.sidebar, top.top_tabs), (Some(SidebarList::SessionsOnly), true));
    assert!(places(TabStyle::Top, false).top_tabs, "top tabs stay with the sidebar hidden");
    assert_eq!(TabStyle::Sidebar.toggled(), TabStyle::Top);
    assert_eq!(TabStyle::Top.toggled(), TabStyle::Sidebar);
}

#[gpui::test]
fn the_toggle_moves_tabs_from_the_sidebar_to_the_titlebar(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    cx.simulate_keystrokes("cmd-t");
    assert_eq!(ws.read_with(cx, |ws, _| ws.settings().tab_style), TabStyle::Sidebar);
    assert!(drawn(cx, "row-0-0") && drawn(cx, "row-0-1"), "each tab is a sidebar row");
    assert!(!drawn(cx, "tab-0"), "no top tab bar in sidebar style");

    act(&ws, cx, "client.tab_style.toggle");
    assert_eq!(ws.read_with(cx, |ws, _| ws.settings().tab_style), TabStyle::Top);
    assert!(drawn(cx, "tab-0") && drawn(cx, "tab-1"), "the tabs are in the titlebar");
    assert!(!drawn(cx, "row-0-0"), "and no longer sidebar rows");
    assert!(drawn(cx, "session-0"), "the sidebar still lists the session");
    let (title, tab) = (at(cx, "titlebar").unwrap(), at(cx, "tab-0").unwrap());
    assert!(title.contains(&center(tab)), "the tabs sit in the titlebar row");
    assert!(drawn(cx, "titlebar-session"), "beside the session name");

    cx.simulate_keystrokes("cmd-shift-s");
    assert_eq!(ws.read_with(cx, |ws, _| ws.settings().tab_style), TabStyle::Sidebar, "⌘⇧S toggles back");
    assert!(drawn(cx, "row-0-1") && !drawn(cx, "tab-1"));
    s.app.stop();
}

/// Every (style, sidebar) state, reached by toggling in turn, draws tabs in at most one place.
#[gpui::test]
fn no_state_draws_tabs_twice(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let mut check = |cx: &mut gpui::VisualTestContext, want_rows: bool, want_top: bool, why: &str| {
        let (rows, top) = (drawn(cx, "row-0-0"), drawn(cx, "tab-0"));
        assert!(!(rows && top), "{why}: tabs in two places");
        assert_eq!((rows, top), (want_rows, want_top), "{why}");
    };
    check(cx, true, false, "sidebar style");
    cx.simulate_keystrokes("cmd-b");
    check(cx, false, false, "sidebar style, sidebar hidden: tabs by ⌘1…9 and the palette");
    assert!(drawn(cx, "titlebar"), "a hidden sidebar leaves a titlebar row for the traffic lights");
    cx.simulate_keystrokes("cmd-shift-s");
    check(cx, false, true, "top style, sidebar hidden");
    assert!(!drawn(cx, "sidebar"), "the toggle keeps the sidebar hidden");
    cx.simulate_keystrokes("cmd-b");
    check(cx, false, true, "top style, sidebar shown");
    assert!(drawn(cx, "session-0"));
    cx.simulate_keystrokes("cmd-shift-s");
    check(cx, true, false, "back to sidebar style");
    s.app.stop();
}

#[gpui::test]
fn the_toggle_is_in_the_palette_and_bindable_from_lua(cx: &mut TestAppContext) {
    let s = setup_with_config(r#"leon.bind{ key = "ctrl+alt+t", action = "client.tab_style.toggle" }"#);
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.sync_keymap());
    cx.simulate_keystrokes("cmd-shift-p");
    cx.simulate_input("tab style");
    let rows = ws.read_with(cx, |ws, _| ws.palette_rows());
    assert!(rows.first().is_some_and(|(t, _)| t == "Toggle Tab Style"), "{rows:?}");
    cx.simulate_keystrokes("escape");
    cx.simulate_keystrokes("ctrl-alt-t");
    assert_eq!(ws.read_with(cx, |ws, _| ws.settings().tab_style), TabStyle::Top, "the Lua binding runs it");
    s.app.stop();
}
