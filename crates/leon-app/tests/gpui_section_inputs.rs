//! ADR R3/R4 (docs/architecture/gpui.md), phase 3: the sidebar, the title bar and each pane header
//! are cached views; each renders again when something it draws changes (one test per input).
//! A host's state is in the sidebar's input too, but no test here starts a host (an ssh); the
//! debug check (a section's render compares its input with a fresh one) covers it.
mod grid_support;

use gpui::{Modifiers, MouseButton, TestAppContext, VisualTestContext};
use grid_support::*;
use serde_json::json;

fn sid(ws: &gpui::Entity<leon_app::view::Workspace>, cx: &mut VisualTestContext) -> String {
    ws.read_with(cx, |ws, _| ws.session_id()).unwrap()
}

#[gpui::test]
fn the_sidebar_renders_on_a_session_title(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let sid = sid(&ws, cx);
    let again = renders_again(&ws, cx, "sidebar", |_| {
        s.app.call("session.set_label", json!({ "session_id": sid, "label": "renamed" })).unwrap();
    });
    s.app.stop();
    assert!(again);
}

#[gpui::test]
fn the_sidebar_renders_on_a_tab_s_status(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let id = tab_running(
        &s,
        &ws,
        cx,
        r"stty -echo; echo ready; read a; printf '\033]7501;state=blocked\007'; exec sleep 300",
        "ready",
    );
    let again = renders_again(&ws, cx, "sidebar", |_| term(&s.app, &id).write(b"\n").unwrap());
    s.app.stop();
    assert!(again);
}

#[gpui::test]
fn the_sidebar_renders_on_the_selection(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let again = renders_again(&ws, cx, "sidebar", |cx| cx.simulate_keystrokes("cmd-n"));
    s.app.stop();
    assert!(again);
}

#[gpui::test]
fn the_sidebar_renders_on_a_collapse(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let label = center(at(cx, "session-0").expect("the session's label"));
    let again = renders_again(&ws, cx, "sidebar", |cx| cx.simulate_click(label, Modifiers::none()));
    let collapsed = !drawn(cx, "row-0-0");
    s.app.stop();
    assert!(again && collapsed, "the selected session's label collapses it");
}

#[gpui::test]
fn the_sidebar_renders_on_its_width(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let edge = center(at(cx, "sidebar-edge").expect("a resize edge"));
    let again = renders_again(&ws, cx, "sidebar", |cx| {
        cx.simulate_mouse_down(edge, MouseButton::Left, Modifiers::none());
        let to = gpui::point(edge.x + gpui::px(60.), edge.y);
        cx.simulate_mouse_move(to, Some(MouseButton::Left), Modifiers::none());
        cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
    });
    s.app.stop();
    assert!(again);
}

#[gpui::test]
fn the_sidebar_renders_on_the_activity_list(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let again = renders_again(&ws, cx, "sidebar", |cx| act(&ws, cx, "client.sidebar.activity.toggle"));
    let shown = drawn(cx, "activity");
    s.app.stop();
    assert!(again, "the sidebar rendered again");
    assert!(shown, "with the Activity list");
}

/// The title bar shows in top tab style.
fn top_tabs(ws: &gpui::Entity<leon_app::view::Workspace>, cx: &mut VisualTestContext) {
    act(ws, cx, "client.tab_style.toggle");
    assert!(shows(cx, |cx| drawn(cx, "titlebar")), "the title bar shows");
}

#[gpui::test]
fn the_title_bar_renders_on_its_session_s_title(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    top_tabs(&ws, cx);
    let sid = sid(&ws, cx);
    let again = renders_again(&ws, cx, "titlebar", |_| {
        s.app.call("session.set_label", json!({ "session_id": sid, "label": "renamed" })).unwrap();
    });
    s.app.stop();
    assert!(again);
}

#[gpui::test]
fn the_title_bar_renders_on_a_new_tab_and_the_active_one(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    top_tabs(&ws, cx);
    let sid = sid(&ws, cx);
    let first = ws.read_with(cx, |ws, _| ws.active_window()).unwrap().id;
    let added = renders_again(&ws, cx, "titlebar", |_| {
        s.app.call("session.new_window", json!({ "session_id": sid })).unwrap();
    });
    assert!(shows(cx, |cx| drawn(cx, "tab-1")), "the new tab shows");
    let switched = renders_again(&ws, cx, "titlebar", |_| {
        s.app.call("session.focus_window", json!({ "session_id": sid, "window_id": first })).unwrap();
    });
    s.app.stop();
    assert!(added && switched);
}

#[gpui::test]
fn the_title_bar_renders_on_a_tab_s_status(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    top_tabs(&ws, cx);
    let id = tab_running(
        &s,
        &ws,
        cx,
        r"stty -echo; echo ready; read a; printf '\033]7501;state=blocked\007'; exec sleep 300",
        "ready",
    );
    let again = renders_again(&ws, cx, "titlebar", |_| term(&s.app, &id).write(b"\n").unwrap());
    s.app.stop();
    assert!(again);
}

const LATER_STATE: &str = r"stty -echo; echo ready; read a; printf '\033]7501;state=blocked:msg=d2FpdGluZw==\007'; read b; printf '\033]0;my-title\007'; exec sleep 300";

#[gpui::test]
fn a_header_renders_on_its_state_caption_and_title(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let id = tab_running(&s, &ws, cx, LATER_STATE, "ready");
    let name = format!("header-{id}");
    let t = term(&s.app, &id);
    let state = renders_again(&ws, cx, &name, |_| t.write(b"\n").unwrap());
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.pane_caption(&id)).is_some()), "a caption came");
    let title = renders_again(&ws, cx, &name, |_| t.write(b"\n").unwrap());
    s.app.stop();
    assert!(state && title);
}

#[gpui::test]
fn headers_render_on_the_focus_and_its_fade(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let left = tab_running(&s, &ws, cx, &quiet("left-ready"), "left-ready");
    let right = split_running(&s, &ws, cx, &left, &quiet("right-ready"), "right-ready");
    let sid = sid(&ws, cx);
    cx.run_until_parked();
    let (l, r) =
        (section_renders(&ws, cx, &format!("header-{left}")), section_renders(&ws, cx, &format!("header-{right}")));
    s.app.call("session.focus_block", json!({ "session_id": sid, "block_id": left })).unwrap();
    let both = shows(cx, |cx| {
        section_renders(&ws, cx, &format!("header-{left}")) > l
            && section_renders(&ws, cx, &format!("header-{right}")) > r
    });
    s.app.stop();
    assert!(both, "the header that got the focus and the one that lost it (and faded)");
}

#[gpui::test]
fn a_header_renders_on_its_find_bar(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let id = tab_running(&s, &ws, cx, &quiet("find-ready"), "find-ready");
    let opened = renders_again(&ws, cx, &format!("header-{id}"), |cx| cx.simulate_keystrokes("cmd-f"));
    let typed = renders_again(&ws, cx, &format!("header-{id}"), |cx| cx.simulate_keystrokes("f i n d"));
    s.app.stop();
    assert!(opened && typed);
}

/// The skeptic's case: a quiet pane's header, reused from its cache while the root drew, shows its
/// controls on hover, and they work.
#[gpui::test]
fn an_unfocused_pane_s_header_shows_working_controls_on_hover(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let old = tab_running(&s, &ws, cx, &quiet("old-ready"), "old-ready");
    let new = split_running(&s, &ws, cx, &old, &quiet("new-ready"), "new-ready");
    assert_eq!(ws.read_with(cx, |ws, _| ws.target_block()), Some(new.clone()));
    // the root draws again while the old header stays cached
    act(&ws, cx, "client.font.increase");
    act(&ws, cx, "client.font.decrease");
    let title = center(at(cx, &format!("header-title-{old}")).expect("the old pane's header title"));
    cx.simulate_mouse_move(title, None::<MouseButton>, Modifiers::none());
    cx.run_until_parked();
    let close = center(at(cx, &format!("header-close-{old}")).expect("its close control"));
    cx.simulate_click(close, Modifiers::none());
    let closed = shows(cx, |cx| ws.read_with(cx, |ws, _| ws.pane_title(&old)).is_none());
    s.app.stop();
    assert!(closed, "hovering the unfocused pane showed its close control, and a click closed it");
}

#[gpui::test]
fn every_section_renders_on_the_theme(cx: &mut TestAppContext) {
    let s = setup_with_config("leon.set(\"theme\", { dark = \"leon-dark\", light = \"leon-light\" })\n");
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, cx| ws.sync_keymap(cx));
    let id = tab_running(&s, &ws, cx, &quiet("theme-ready"), "theme-ready");
    let dark = ws.read_with(cx, |ws, _| ws.theme()) == leon_app::theme::DARK;
    let (sidebar, header) = (section_renders(&ws, cx, "sidebar"), section_renders(&ws, cx, &format!("header-{id}")));
    ws.update_in(cx, |ws, _, cx| ws.set_appearance(!dark, cx));
    let both = shows(cx, |cx| {
        section_renders(&ws, cx, "sidebar") > sidebar && section_renders(&ws, cx, &format!("header-{id}")) > header
    });
    s.app.stop();
    assert!(both, "the sidebar and the header drew the new theme");
}

#[gpui::test]
fn the_title_bar_renders_when_the_sidebar_hides(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    top_tabs(&ws, cx);
    // the toolbar's buttons move into the title bar
    let again = renders_again(&ws, cx, "titlebar", |cx| cx.simulate_keystrokes("cmd-b"));
    let moved = drawn(cx, "toolbar-palette");
    s.app.stop();
    assert!(again && moved);
}

#[gpui::test]
fn a_header_renders_when_its_pane_gets_narrower(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let id = tab_running(&s, &ws, cx, &quiet("width-ready"), "width-ready");
    // the title is cut to the pane's width, an input that a resize changes
    let again = renders_again(&ws, cx, &format!("header-{id}"), |cx| {
        cx.simulate_resize(gpui::size(gpui::px(700.), gpui::px(600.)))
    });
    s.app.stop();
    assert!(again);
}
