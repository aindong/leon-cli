//! M16b criterion 9: the empty state is bigger and better balanced in a big window, and offers
//! "Add a host".

mod m10b_support;

use gpui::{Modifiers, TestAppContext};
use m10b_support::*;

#[gpui::test]
fn the_empty_state_is_larger_and_adds_a_host(cx: &mut TestAppContext) {
    let s = setup_with_config(r#"leon.bind("cmd+k", "client.palette.toggle")"#);
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.sync_keymap());
    cx.simulate_keystrokes("cmd-w");
    assert!(drawn(cx, "empty-state"));
    let hints = ws.read_with(cx, |ws, _| ws.empty_hints());
    assert_eq!(hints.len(), 4, "{hints:?}");
    assert_eq!(hints[3], ("leon hosts add".to_string(), "Add a host".to_string()));

    let mark = at(cx, "empty-mark").expect("the mark");
    assert!(mark.size.height >= gpui::px(120.), "the mark is big: {mark:?}");
    let title = at(cx, "empty-title").expect("the title");
    assert!(title.size.height >= gpui::px(36.), "the title is big: {title:?}");
    let row = at(cx, "hint-0").expect("a hint row");
    assert!(row.size.height >= gpui::px(40.), "hint rows are tall: {row:?}");
    let content = at(cx, "empty-content").expect("the content block");
    assert!(content.size.width >= gpui::px(420.), "the hints use the room: {content:?}");

    let add = at(cx, "hint-3").unwrap();
    cx.simulate_click(center(add), Modifiers::none());
    assert!(ws.read_with(cx, |ws, _| ws.palette_open()), "Add a host opens the prompt");
    let rows = ws.read_with(cx, |ws, _| ws.palette_rows());
    assert!(rows.iter().any(|(t, _)| t.to_lowercase().contains("label")), "{rows:?}");
    s.app.stop();
}
