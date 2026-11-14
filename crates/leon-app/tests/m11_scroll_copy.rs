//! M11 criterion 6: a 10000-line block scrolled by wheel, trackpad and keys, the "↓ N new lines"
//! affordance, and selections copied with cmd+c, copy-on-select, cmd+a.
mod m11_support;

use gpui::{point, px, Modifiers, ScrollDelta, ScrollWheelEvent, TestAppContext, TouchPhase};
use m11_support::*;

/// 10000 lines, then 5 more on each Enter (no echo, so only those lines are pushed).
const SCRIPT: &str = "stty -echo; seq 1 10000; read a; seq 10001 10005; read b; seq 10006 10010; sleep 100";

fn wheel(cx: &mut gpui::VisualTestContext, at: gpui::Point<gpui::Pixels>, delta: ScrollDelta, phase: TouchPhase) {
    cx.simulate_event(ScrollWheelEvent { position: at, delta, modifiers: Modifiers::none(), touch_phase: phase });
    cx.run_until_parked();
}

fn first(ws: &gpui::Entity<leon_app::view::Workspace>, cx: &mut gpui::VisualTestContext, id: &str) -> usize {
    view_lines(ws, cx, id)[0].parse().expect("a number on the first row")
}

#[gpui::test]
fn wheel_trackpad_keys_and_the_new_lines_affordance(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let id = tab_running(&s, &ws, cx, SCRIPT, "10000");
    let bottom = first(&ws, cx, &id);
    let at = cell(&ws, cx, &id, 3, 3);

    wheel(cx, at, ScrollDelta::Lines(point(0., 3.)), TouchPhase::Moved);
    assert_eq!(first(&ws, cx, &id), bottom - 3, "three lines up, fetched from the scrollback");
    assert!(ws.read_with(cx, |ws, _| ws.view_top(&id)).is_some());

    let lh = line_height(&ws, cx, &id);
    for (i, phase) in
        [TouchPhase::Started].into_iter().chain([TouchPhase::Moved; 8]).chain([TouchPhase::Ended]).enumerate()
    {
        let _ = i;
        wheel(cx, at, ScrollDelta::Pixels(point(px(0.), lh * 0.3)), phase);
    }
    assert_eq!(first(&ws, cx, &id), bottom - 6, "ten 0.3-line trackpad deltas are three lines");

    // output while scrolled up: the same lines stay, the affordance counts the new ones
    focused(&s.app, &ws, cx).write(b"\n").unwrap();
    let counted = shows(cx, |cx| ws.read_with(cx, |ws, _| ws.new_lines(&id)) == Some(5));
    assert!(counted, "↓ 5 new lines: {:?}", ws.read_with(cx, |ws, _| ws.new_lines(&id)));
    assert_eq!(first(&ws, cx, &id), bottom - 6, "the view held still");
    let jump = at_sel(cx, &format!("jump-{id}")).expect("the affordance is drawn");
    cx.simulate_click(center(jump), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(ws.read_with(cx, |ws, _| (ws.view_top(&id), ws.new_lines(&id))), (None, None), "back at the bottom");
    assert!(view_lines(&ws, cx, &id).contains(&"10005".to_string()));

    let rows = view_lines(&ws, cx, &id).len();
    let now = first(&ws, cx, &id);
    cx.simulate_keystrokes("shift-pageup");
    cx.run_until_parked();
    let paged = first(&ws, cx, &id);
    assert!(now - paged >= rows - 2 && now - paged <= rows, "a page up: {now} → {paged} with {rows} rows");
    cx.simulate_keystrokes("shift-pagedown");
    cx.run_until_parked();
    assert_eq!(first(&ws, cx, &id), now);
    cx.simulate_keystrokes("cmd-up");
    cx.run_until_parked();
    let top = first(&ws, cx, &id);
    assert!(top <= 10005 - 10000 + 1, "the oldest kept line is on top: {top}");
    cx.simulate_keystrokes("cmd-down");
    cx.run_until_parked();
    assert_eq!(ws.read_with(cx, |ws, _| ws.view_top(&id)), None);
    s.app.stop();
}

#[gpui::test]
fn drag_word_line_extend_rectangle_and_select_all(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let id = tab_running(&s, &ws, cx, SCRIPT, "10000");
    let r = row_of(&ws, cx, &id, "9998");

    let (from, to) = (cell(&ws, cx, &id, r, 0), cell(&ws, cx, &id, r + 1, 3));
    drag(cx, from, to, Modifiers::none());
    assert_eq!(clipboard(cx), None, "no copy-on-select by default");
    cx.simulate_keystrokes("cmd-c");
    assert_eq!(clipboard(cx).as_deref(), Some("9998\n9999"));

    // the selection stays on its text while output arrives
    focused(&s.app, &ws, cx).write(b"\n").unwrap();
    assert!(shows(cx, |cx| view_lines(&ws, cx, &id).contains(&"10005".to_string())));
    assert_eq!(ws.read_with(cx, |ws, _| ws.highlighted_text(&id)).as_deref(), Some("9998\n9999"));

    let r = row_of(&ws, cx, &id, "10003");
    let word = cell(&ws, cx, &id, r, 2);
    press(cx, word, Modifiers::none(), 2);
    release(cx, word, Modifiers::none(), 2);
    cx.simulate_keystrokes("cmd-c");
    assert_eq!(clipboard(cx).as_deref(), Some("10003"), "double-click: the word");

    press(cx, word, Modifiers::none(), 3);
    release(cx, word, Modifiers::none(), 3);
    cx.simulate_keystrokes("cmd-c");
    assert_eq!(clipboard(cx).map(|t| t.trim_end().to_string()).as_deref(), Some("10003"), "triple-click: the line");

    let start = cell(&ws, cx, &id, r, 1);
    press(cx, start, Modifiers::none(), 1);
    release(cx, start, Modifiers::none(), 1);
    let end = cell(&ws, cx, &id, r + 1, 2);
    press(cx, end, shift(), 1);
    release(cx, end, shift(), 1);
    cx.simulate_keystrokes("cmd-c");
    assert_eq!(clipboard(cx).as_deref(), Some("0003\n100"), "shift+click extends");

    let (from, to) = (cell(&ws, cx, &id, r, 3), cell(&ws, cx, &id, r + 1, 4));
    drag(cx, from, to, alt());
    cx.simulate_keystrokes("cmd-c");
    assert_eq!(clipboard(cx).as_deref(), Some("03\n04"), "option+drag: a rectangle");

    cx.simulate_keystrokes("cmd-a");
    cx.simulate_keystrokes("cmd-c");
    let all = clipboard(cx).unwrap();
    assert!(all.lines().count() >= 9000, "the whole scrollback: {} lines", all.lines().count());
    assert!(all.contains("\n5000\n") && all.trim_end().ends_with("10005"));
    s.app.stop();
}

#[gpui::test]
fn copy_on_select_when_the_config_says_so(cx: &mut TestAppContext) {
    let s = setup_with_config("leon.set(\"copy_on_select\", true)\n");
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.sync_keymap());
    let id = tab_running(&s, &ws, cx, SCRIPT, "10000");
    let r = row_of(&ws, cx, &id, "9999");
    let (from, to) = (cell(&ws, cx, &id, r, 0), cell(&ws, cx, &id, r, 3));
    drag(cx, from, to, Modifiers::none());
    assert_eq!(clipboard(cx).as_deref(), Some("9999"), "the drag alone copied");
    s.app.stop();
}

/// The bounds of a drawn element, by its debug selector.
fn at_sel(cx: &mut gpui::VisualTestContext, sel: &str) -> Option<gpui::Bounds<gpui::Pixels>> {
    m11_support::at(cx, sel)
}
