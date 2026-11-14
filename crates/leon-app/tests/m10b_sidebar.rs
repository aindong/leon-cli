//! M10b criterion 3: a quiet sidebar. Session labels are small and muted, tab rows are 30 pt with
//! one status glyph, ⌘B hides it, and dragging its edge resizes it (clamped, and saved).

mod m10b_support;

use gpui::{point, px, Modifiers, MouseButton, TestAppContext};
use leon_app::status::Indicator;
use m10b_support::*;

const WORKING: &str = r"printf '\033]7501;state=working\007'; echo w-sent; sleep 60";

#[gpui::test]
fn rows_are_30pt_and_labels_are_smaller(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let row = at(cx, "row-0-0").expect("a tab row");
    assert_eq!(f32::from(row.size.height), 30., "tab rows are 30 pt");
    let label = at(cx, "session-0").expect("a session label");
    assert!(label.size.height < row.size.height, "a session label is quieter than a row: {label:?}");
    assert!(!drawn(cx, "session-count-0"), "no count pill");
    run_until(&focused(&s.app, &ws, cx), WORKING, "w-sent");
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.indicators()).1
        == vec![Some(Indicator::Working { progress: None })]));
    assert!(drawn(cx, "row-status-0-0"), "the row shows the status glyph");
    s.app.stop();
}

#[gpui::test]
fn cmd_b_hides_and_the_edge_drag_resizes(cx: &mut TestAppContext) {
    let s = setup();
    let dir = private_dir();
    let path = dir.path().join("app.json");
    let (ws, cx) = open_with(cx, &s.app, Some(path.clone()));
    let side = at(cx, "sidebar").expect("the sidebar");
    assert_eq!(f32::from(side.size.width), 220.);
    let edge = center(at(cx, "sidebar-edge").expect("a resize edge"));
    cx.simulate_mouse_down(edge, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(point(edge.x + px(80.), edge.y), Some(MouseButton::Left), Modifiers::none());
    cx.simulate_mouse_up(point(edge.x + px(80.), edge.y), MouseButton::Left, Modifiers::none());
    let wide = f32::from(at(cx, "sidebar").unwrap().size.width);
    assert!((wide - 300.).abs() <= 1., "the drag widened it to ~300: {wide}");
    assert_eq!(ws.read_with(cx, |ws, _| ws.settings().sidebar_width), wide);
    assert_eq!(leon_app::chrome::Settings::load(&path).sidebar_width, wide, "and saved the width");

    let edge = center(at(cx, "sidebar-edge").unwrap());
    cx.simulate_mouse_down(edge, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(point(edge.x + px(900.), edge.y), Some(MouseButton::Left), Modifiers::none());
    cx.simulate_mouse_up(point(edge.x + px(900.), edge.y), MouseButton::Left, Modifiers::none());
    assert_eq!(f32::from(at(cx, "sidebar").unwrap().size.width), 400., "clamped to 400");

    cx.simulate_keystrokes("cmd-b");
    assert!(!drawn(cx, "sidebar"), "⌘B hides it");
    cx.simulate_keystrokes("cmd-b");
    assert!(drawn(cx, "sidebar"));
    s.app.stop();
}
