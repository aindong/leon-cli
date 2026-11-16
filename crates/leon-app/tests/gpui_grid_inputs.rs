//! ADR R3/R4 (docs/architecture/gpui.md), phase 2: a pane's grid is a cached view, notified when
//! something it draws changes. Each test changes one input and checks the grid rendered again
//! with it (`grid_drew`, what its last render drew). The window's activation and the theme
//! re-render every cached view anyway (GPUI refreshes the window; the text style is the cache's
//! key), so those two check what the grid drew, not that it rendered.
mod grid_support;

use std::time::{Duration, Instant};

use gpui::{Entity, Modifiers, MouseButton, TestAppContext, VisualTestContext};
use grid_support::*;
use leon_app::view::Workspace;
use serde_json::json;

const LINES: &str = "stty -echo; seq 1 3000; echo seq-done; read a; echo marker-$a; sleep 100";

/// Whether pane `id`'s grid renders again within 10 s after `change`, and what it drew then.
fn renders_again(
    ws: &Entity<Workspace>,
    cx: &mut VisualTestContext,
    id: &str,
    change: impl FnOnce(&mut VisualTestContext),
) -> Option<leon_app::view::GridDrawn> {
    cx.run_until_parked();
    let before = grid_renders(ws, cx, id);
    change(cx);
    let end = Instant::now() + Duration::from_secs(10);
    while Instant::now() < end {
        cx.run_until_parked();
        if grid_renders(ws, cx, id) > before {
            return ws.read_with(cx, |ws, _| ws.grid_drew(id));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    None
}

#[gpui::test]
fn a_pushed_screen_renders_its_grid(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let id = tab_running(&s, &ws, cx, LINES, "seq-done");
    let t = term(&s.app, &id);
    let drew = renders_again(&ws, cx, &id, |_| t.write(b"x\n").unwrap());
    s.app.stop();
    assert!(drew.is_some_and(|d| d.text.contains("marker-x")), "the grid drew the new screen");
}

#[gpui::test]
fn the_scroll_offset_renders_its_grid(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let id = tab_running(&s, &ws, cx, LINES, "seq-done");
    let drew = renders_again(&ws, cx, &id, |cx| act(&ws, cx, "client.scroll.top"));
    s.app.stop();
    assert!(drew.is_some_and(|d| d.text.lines().next() == Some("1")), "the grid drew the top of the history");
}

#[gpui::test]
fn the_selection_renders_its_grid(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let id = tab_running(&s, &ws, cx, LINES, "seq-done");
    let drew = renders_again(&ws, cx, &id, |cx| act(&ws, cx, "pane.select_all"));
    s.app.stop();
    assert!(drew.is_some_and(|d| d.marks > 0), "the grid drew the selection");
}

#[gpui::test]
fn find_matches_render_its_grid(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let id = tab_running(&s, &ws, cx, LINES, "seq-done");
    cx.simulate_keystrokes("cmd-f");
    let drew = renders_again(&ws, cx, &id, |cx| cx.simulate_keystrokes("s e q -"));
    s.app.stop();
    assert!(drew.is_some_and(|d| d.marks > 0), "the grid drew the match");
}

#[gpui::test]
fn stepping_to_the_next_match_renders_its_grid(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let script = "stty -echo; printf 'zq-one\\nzq-two\\n'; sleep 100";
    let id = tab_running(&s, &ws, cx, script, "zq-two");
    cx.simulate_keystrokes("cmd-f z q -");
    assert!(
        shows(cx, |cx| ws.read_with(cx, |ws, _| ws.find_status()) == Some((2, 2))),
        "two matches, the newest current"
    );
    let before = ws.read_with(cx, |ws, _| ws.grid_drew(&id)).map(|d| d.marks);
    let drew = renders_again(&ws, cx, &id, |cx| cx.simulate_keystrokes("enter"));
    let status = ws.read_with(cx, |ws, _| ws.find_status());
    s.app.stop();
    assert_eq!(status, Some((1, 2)), "enter stepped to the other match");
    assert!(drew.is_some_and(|d| Some(d.marks) == before), "the grid drew the current match moved");
}

#[gpui::test]
fn the_cmd_hover_link_renders_its_grid(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let script = "stty -echo; printf 'go https://example.com/page here\\n'; sleep 100";
    let id = tab_running(&s, &ws, cx, script, "example.com");
    let row = row_of(&ws, cx, &id, "go https://example.com/page here");
    let at = cell(&ws, cx, &id, row, 8);
    cx.simulate_mouse_move(at, None::<MouseButton>, Modifiers::none());
    let drew = renders_again(&ws, cx, &id, |cx| cx.simulate_modifiers_change(cmd()));
    let link = ws.read_with(cx, |ws, _| ws.hovered_link());
    s.app.stop();
    assert_eq!(link.as_deref(), Some("https://example.com/page"));
    assert!(drew.is_some_and(|d| d.marks > 0), "the grid drew the underline");
}

#[gpui::test]
fn copy_mode_renders_its_grid(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let id = tab_running(&s, &ws, cx, LINES, "seq-done");
    let drew = renders_again(&ws, cx, &id, |cx| act(&ws, cx, "client.copy_mode"));
    s.app.stop();
    assert!(drew.is_some_and(|d| d.text.contains("COPY")), "the grid drew copy mode's badge");
}

#[gpui::test]
fn hints_render_its_grid(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let script = "stty -echo; printf 'see https://example.com/a and /tmp/x\\n'; sleep 100";
    let id = tab_running(&s, &ws, cx, script, "example.com");
    let drew = renders_again(&ws, cx, &id, |cx| act(&ws, cx, "client.hints"));
    let labels = ws.read_with(cx, |ws, _| ws.hint_labels(&id));
    s.app.stop();
    let label = labels.first().map(|(l, _)| l.clone()).expect("hints are on");
    assert!(drew.is_some_and(|d| d.text.contains(&label)), "the grid drew the label {label:?}");
}

#[gpui::test]
fn the_font_size_renders_its_grid(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let id = tab_running(&s, &ws, cx, LINES, "seq-done");
    let size = ws.read_with(cx, |ws, _| ws.font_size());
    let drew = renders_again(&ws, cx, &id, |cx| act(&ws, cx, "client.font.increase"));
    s.app.stop();
    assert!(drew.is_some_and(|d| d.font_size > size), "the grid drew the larger font");
}

/// Two panes side by side; returns (left, right), the right one focused.
fn two_panes(s: &Setup, ws: &Entity<Workspace>, cx: &mut VisualTestContext) -> (String, String) {
    let left = tab_running(s, ws, cx, LINES, "seq-done");
    let right = split_running(s, ws, cx, &left, LINES, "seq-done");
    (left, right)
}

#[gpui::test]
fn the_focus_moving_to_a_pane_renders_its_grid(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    cx.update(|window, _| window.activate_window());
    let (left, _) = two_panes(&s, &ws, cx);
    let sid = ws.read_with(cx, |ws, _| ws.session_id()).unwrap();
    let drew = renders_again(&ws, cx, &left, |_| {
        s.app.call("session.focus_block", json!({ "session_id": sid, "block_id": left })).unwrap();
    });
    s.app.stop();
    assert!(drew.is_some_and(|d| d.active), "the grid drew the focused cursor");
}

#[gpui::test]
fn the_fade_level_renders_its_grid(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.set_reduce_motion(true)); // a fade lands at once
    cx.update(|window, _| window.activate_window());
    let (left, right) = two_panes(&s, &ws, cx);
    let sid = ws.read_with(cx, |ws, _| ws.session_id()).unwrap();
    let drew = renders_again(&ws, cx, &right, |_| {
        s.app.call("session.focus_block", json!({ "session_id": sid, "block_id": left })).unwrap();
    });
    let (theme, window) = ws.read_with(cx, |ws, _| (ws.pane_theme(&right), ws.theme()));
    s.app.stop();
    assert_ne!(theme, window, "the pane that lost the focus is faded");
    assert!(drew.is_some_and(|d| d.theme == theme), "the grid drew it faded");
}

#[gpui::test]
fn the_window_s_activation_is_what_the_grid_draws(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    cx.update(|window, _| window.activate_window());
    let id = tab_running(&s, &ws, cx, LINES, "seq-done");
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.grid_drew(&id)).is_some_and(|d| d.active)));
    cx.deactivate_window();
    let inactive = shows(cx, |cx| ws.read_with(cx, |ws, _| ws.grid_drew(&id)).is_some_and(|d| !d.active));
    s.app.stop();
    assert!(inactive, "the grid drew the cursor of a window that isn't key");
}

#[gpui::test]
fn the_theme_is_what_the_grid_draws(cx: &mut TestAppContext) {
    let s = setup_with_config("leon.set(\"theme\", { dark = \"leon-dark\", light = \"leon-light\" })\n");
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.sync_keymap());
    let id = tab_running(&s, &ws, cx, LINES, "seq-done");
    let before = ws.read_with(cx, |ws, _| ws.theme());
    ws.update_in(cx, |ws, _, cx| ws.set_appearance(false, cx));
    if ws.read_with(cx, |ws, _| ws.theme()) == before {
        ws.update_in(cx, |ws, _, cx| ws.set_appearance(true, cx));
    }
    assert_ne!(ws.read_with(cx, |ws, _| ws.theme()), before, "the appearance changed the theme");
    let theme = ws.read_with(cx, |ws, _| ws.pane_theme(&id));
    let drawn = shows(cx, |cx| ws.read_with(cx, |ws, _| ws.grid_drew(&id)).is_some_and(|d| d.theme == theme));
    s.app.stop();
    assert!(drawn, "the grid drew the new appearance's theme");
}
