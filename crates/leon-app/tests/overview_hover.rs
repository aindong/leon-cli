//! Overview criterion 15 (.agent/plans/2026-11-17-overview-grid.md): resting on a tab shows a live
//! thumbnail of it; moving to another tab moves it at once; leaving hides it. In the titlebar and
//! in the sidebar alike.

mod m10b_support;

use std::time::Duration;

use gpui::{Entity, Modifiers, TestAppContext, VisualTestContext};
use leon_app::view::Workspace;
use m10b_support::*;

fn hover(cx: &mut VisualTestContext, sel: &str) {
    let b = at(cx, sel).unwrap_or_else(|| panic!("{sel} is drawn"));
    cx.simulate_mouse_move(center(b), None, Modifiers::none());
    cx.run_until_parked();
}

fn previewed(ws: &Entity<Workspace>, cx: &mut VisualTestContext) -> Option<String> {
    cx.run_until_parked();
    drawn(cx, "hover-preview").then(|| ws.read_with(cx, |ws, _| ws.hover_preview())).flatten()
}

fn check(ws: &Entity<Workspace>, cx: &mut VisualTestContext, first: &str, second: &str, tabs: &[String]) {
    hover(cx, first);
    assert_eq!(previewed(ws, cx), None, "no preview before the pointer rests");
    cx.executor().advance_clock(Duration::from_millis(450));
    assert_eq!(previewed(ws, cx), Some(tabs[0].clone()), "a rest shows the tab's thumbnail");
    assert!(drawn(cx, &format!("hover-pane-{}", tabs[0])) || drawn(cx, "hover-preview"));
    hover(cx, second);
    assert_eq!(previewed(ws, cx), Some(tabs[1].clone()), "moving to another tab moves it at once");
    hover(cx, "panes-area");
    assert_eq!(previewed(ws, cx), None, "leaving the tabs hides it");
    cx.executor().advance_clock(Duration::from_millis(450));
    assert_eq!(previewed(ws, cx), None, "and it stays hidden");
}

fn two_tabs(ws: &Entity<Workspace>, cx: &mut VisualTestContext) -> Vec<String> {
    act(ws, cx, "client.tab.new");
    let (wins, _) = ws.read_with(cx, |ws, _| ws.windows()).unwrap();
    assert_eq!(wins.len(), 2);
    wins.into_iter().map(|w| w.id).collect()
}

#[gpui::test]
fn titlebar_tabs_preview_on_hover(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    act(&ws, cx, "client.tab_style.toggle");
    let tabs = two_tabs(&ws, cx);
    check(&ws, cx, "tab-0", "tab-1", &tabs);
    s.app.stop();
}

#[gpui::test]
fn sidebar_rows_preview_on_hover(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let tabs = two_tabs(&ws, cx);
    check(&ws, cx, "row-0-0", "row-0-1", &tabs);
    s.app.stop();
}
