//! Overview criteria 1, 2, 4–7 (.agent/plans/2026-11-17-overview-grid.md): ⌘⇧\ fills the window
//! with every tab as a card, ⌘\ with this session's; ↩ and a click go there, esc goes back,
//! typing filters, and a card's status is the sidebar's.

mod m10b_support;

use gpui::{Entity, Modifiers, TestAppContext, VisualTestContext};
use leon_app::view::Workspace;
use m10b_support::*;
use serde_json::json;

const BLOCKED: &str = r"printf '\033]7501;state=blocked\007'; echo b-sent";

/// Three sessions: the first (selected) with tabs `one` and `two`, `ci` with `lint` and `tests`,
/// and `docs` with `site`. Returns (session id, window id) of `tests`.
fn sessions(s: &Setup, ws: &Entity<Workspace>, cx: &mut VisualTestContext) -> (String, String) {
    let first = ws.read_with(cx, |ws, _| ws.session_id()).unwrap();
    let w0 = ws.read_with(cx, |ws, _| ws.active_window()).unwrap().id;
    s.app.call("session.set_window_label", json!({ "session_id": first, "window_id": w0, "label": "one" })).unwrap();
    s.app.call("session.new_window", json!({ "session_id": first, "window_label": "two" })).unwrap();
    s.app.call("session.focus_window", json!({ "session_id": first, "window_id": w0 })).unwrap();
    let ci = s.app.call("session.create", json!({ "label": "ci", "window_label": "lint" })).unwrap();
    let ci_sid = ci["session_id"].as_str().unwrap().to_string();
    let tests = s.app.call("session.new_window", json!({ "session_id": ci_sid, "window_label": "tests" })).unwrap();
    s.app.call("session.create", json!({ "label": "docs", "window_label": "site" })).unwrap();
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.session_count()) == 3), "the app sees three sessions");
    (ci_sid, tests["window_id"].as_str().unwrap().to_string())
}

fn titles(ws: &Entity<Workspace>, cx: &mut VisualTestContext) -> Vec<String> {
    ws.read_with(cx, |ws, _| ws.overview_cards().into_iter().map(|c| c.title).collect())
}

#[gpui::test]
fn every_tab_of_every_session_is_a_card(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let (_, tests) = sessions(&s, &ws, cx);
    assert!(!ws.read_with(cx, |ws, _| ws.overview_open()));
    cx.simulate_keystrokes("cmd-shift-\\");
    assert!(ws.read_with(cx, |ws, _| ws.overview_open()), "cmd+shift+\\ opens the overview");
    assert!(drawn(cx, "overview"), "it is drawn");
    assert_eq!(titles(&ws, cx), ["one", "two", "lint", "tests", "site"], "every tab, in the sidebar's order");
    assert!(drawn(cx, &format!("card-{tests}")), "each card is drawn");
    for label in ["overview-session-0", "overview-session-1", "overview-session-2"] {
        assert!(drawn(cx, label), "{label}: a label per session");
    }
    assert!(!drawn(cx, "sidebar"), "the overview fills the window");
    cx.simulate_keystrokes("cmd-shift-\\");
    assert!(!ws.read_with(cx, |ws, _| ws.overview_open()), "the same keys close it");
    s.app.stop();
}

#[gpui::test]
fn cmd_backslash_shows_only_this_sessions_tabs(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    sessions(&s, &ws, cx);
    cx.simulate_keystrokes("cmd-\\");
    assert!(ws.read_with(cx, |ws, _| ws.overview_open()));
    assert_eq!(titles(&ws, cx), ["one", "two"]);
    cx.simulate_keystrokes("escape");
    // the palette's actions and the toolbar's button
    act(&ws, cx, "client.overview.session.toggle");
    assert_eq!(titles(&ws, cx), ["one", "two"]);
    act(&ws, cx, "client.overview.session.toggle");
    act(&ws, cx, "client.overview.toggle");
    assert_eq!(titles(&ws, cx).len(), 5);
    act(&ws, cx, "client.overview.toggle");
    assert!(!ws.read_with(cx, |ws, _| ws.overview_open()));
    let button = at(cx, "toolbar-overview").expect("a toolbar button");
    cx.simulate_click(center(button), Modifiers::none());
    assert!(ws.read_with(cx, |ws, _| ws.overview_open()), "the button opens it");
    assert_eq!(titles(&ws, cx).len(), 5, "with every tab");
    let builtins = leon_app::palette::BUILTINS;
    for name in ["client.overview.toggle", "client.overview.session.toggle"] {
        assert!(builtins.iter().any(|(n, _)| *n == name), "{name} is in the palette");
    }
    s.app.stop();
}

#[gpui::test]
fn enter_and_a_click_go_to_the_card_and_close(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let (ci, tests) = sessions(&s, &ws, cx);
    cx.simulate_keystrokes("cmd-shift-\\");
    assert_eq!(ws.read_with(cx, |ws, _| ws.overview_selected()), 0, "the current tab is selected");
    cx.simulate_keystrokes("right right right"); // one, two, lint, tests
    cx.simulate_keystrokes("enter");
    assert!(!ws.read_with(cx, |ws, _| ws.overview_open()), "enter closes it");
    let there = |ws: &Workspace| (ws.session_id(), ws.active_window().map(|w| w.id));
    let went = shows(cx, |cx| ws.read_with(cx, |ws, _| there(ws)) == (Some(ci.clone()), Some(tests.clone())));
    assert!(went, "enter went to ci › tests: {:?}", ws.read_with(cx, |ws, _| there(ws)));
    cx.simulate_keystrokes("cmd-shift-\\");
    assert_eq!(ws.read_with(cx, |ws, _| ws.overview_selected()), 3, "opening again selects the current tab's card");
    cx.simulate_keystrokes("escape");

    cx.simulate_keystrokes("cmd-shift-\\");
    let site = ws.read_with(cx, |ws, _| ws.overview_cards()[4].clone());
    let card = at(cx, &format!("card-{}", site.window_id)).expect("the site card is drawn");
    cx.simulate_click(center(card), Modifiers::none());
    assert!(!ws.read_with(cx, |ws, _| ws.overview_open()), "a click closes it");
    let went = shows(cx, |cx| {
        ws.read_with(cx, |ws, _| there(ws)) == (Some(site.session_id.clone()), Some(site.window_id.clone()))
    });
    assert!(went, "the click went to docs › site");
    s.app.stop();
}

#[gpui::test]
fn esc_leaves_everything_as_it_was(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    sessions(&s, &ws, cx);
    let before = ws.read_with(cx, |ws, _| (ws.session_id(), ws.active_window().map(|w| w.id), ws.target_block()));
    cx.simulate_keystrokes("cmd-shift-\\");
    cx.simulate_keystrokes("right down");
    cx.simulate_keystrokes("escape");
    assert!(!ws.read_with(cx, |ws, _| ws.overview_open()), "esc closes it");
    cx.run_until_parked();
    let after = ws.read_with(cx, |ws, _| (ws.session_id(), ws.active_window().map(|w| w.id), ws.target_block()));
    assert_eq!(before, after, "the same session, tab and pane");
    let pane = after.2.unwrap();
    cx.simulate_input("echo back-in-pane");
    let typed = shows(cx, |_| term(&s.app, &pane).text(true).contains("back-in-pane"));
    assert!(typed, "typing after esc reaches the pane");
    s.app.stop();
}

#[gpui::test]
fn typing_filters_the_cards_by_title(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    sessions(&s, &ws, cx);
    cx.simulate_keystrokes("cmd-shift-\\");
    cx.simulate_input("tes");
    assert_eq!(ws.read_with(cx, |ws, _| ws.overview_query()), "tes");
    assert_eq!(titles(&ws, cx), ["tests"]);
    assert_eq!(ws.read_with(cx, |ws, _| ws.overview_selected()), 0, "the selection is on a shown card");
    cx.simulate_keystrokes("backspace backspace backspace");
    assert_eq!(titles(&ws, cx).len(), 5);
    cx.simulate_input("qqqq");
    assert!(titles(&ws, cx).is_empty());
    assert!(drawn(cx, "overview-empty"), "says nothing matches");
    s.app.stop();
}

#[gpui::test]
fn a_cards_status_is_the_sidebars(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let (_, tests) = sessions(&s, &ws, cx);
    let block = s
        .app
        .ctx()
        .state()
        .sessions
        .iter()
        .flat_map(|s| s.blocks.iter())
        .find(|b| b.window_id == tests)
        .map(|b| b.id.clone())
        .unwrap();
    run_until(&term(&s.app, &block), BLOCKED, "b-sent");
    let blocked = |cx: &mut VisualTestContext| {
        ws.read_with(cx, |ws, _| ws.overview_cards().iter().any(|c| c.window_id == tests && c.status.is_some()))
    };
    cx.simulate_keystrokes("cmd-shift-\\");
    assert!(shows(cx, blocked), "the card shows its tab's status");
    let card = ws.read_with(cx, |ws, _| ws.overview_cards().into_iter().find(|c| c.window_id == tests).unwrap());
    // the sidebar's: the ci session's second tab row
    cx.simulate_keystrokes("escape");
    let sidebar = ws.read_with(cx, |ws, _| ws.tab_status(&card.session_id, &tests));
    assert_eq!(card.status, sidebar, "the same indicator as the sidebar's row");
    assert_eq!(card.status, Some(leon_app::status::Indicator::Blocked(None)));
    cx.simulate_keystrokes("cmd-shift-\\");
    assert!(drawn(cx, &format!("card-status-{tests}")), "its mark is drawn");
    assert!(drawn(cx, &format!("card-word-{tests}")), "and its state in words");
    assert_eq!(leon_app::attention::state_text(card.status.unwrap()), "blocked", "Activity's word for it");
    s.app.stop();
}

#[gpui::test]
fn the_menus_actions_still_work_while_it_is_open(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    cx.simulate_keystrokes("cmd-shift-\\");
    assert!(ws.read_with(cx, |ws, _| ws.overview_open()));
    let before = s.app.ctx().state().sessions.iter().map(|s| s.windows.len()).sum::<usize>();
    cx.dispatch_action(leon_app::view::NewTab);
    let made = shows(cx, |_| s.app.ctx().state().sessions.iter().map(|s| s.windows.len()).sum::<usize>() > before);
    assert!(made, "the menu's New Tab reached the workspace under the overview");
    s.app.stop();
}
