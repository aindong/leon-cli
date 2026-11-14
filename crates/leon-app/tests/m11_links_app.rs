//! M11 criterion 9: cmd+hover underlines a link, cmd+click opens a web link at once and asks
//! first for anything else.
mod m11_support;

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{Modifiers, MouseButton, TestAppContext};
use m11_support::*;

const SCRIPT: &str = "stty -echo; printf 'go https://example.com/page here\\nfile:///etc/hosts\\n'; sleep 100";

#[gpui::test]
fn cmd_hover_and_click_open_links_by_policy(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let id = tab_running(&s, &ws, cx, SCRIPT, "file:///etc/hosts");
    let opened = Rc::new(RefCell::new(Vec::<String>::new()));
    let o = opened.clone();
    ws.update(cx, |ws, _| ws.set_opener(Box::new(move |uri: &str| o.borrow_mut().push(uri.to_string()))));
    let (web_row, file_row) =
        (row_of(&ws, cx, &id, "go https://example.com/page here"), row_of(&ws, cx, &id, "file:///etc/hosts"));
    let (web, file) = (cell(&ws, cx, &id, web_row, 8), cell(&ws, cx, &id, file_row, 3));

    cx.simulate_mouse_move(web, None::<MouseButton>, Modifiers::none());
    assert_eq!(ws.read_with(cx, |ws, _| ws.hovered_link()), None, "no underline without cmd");
    cx.simulate_modifiers_change(cmd());
    cx.run_until_parked();
    assert_eq!(ws.read_with(cx, |ws, _| ws.hovered_link()).as_deref(), Some("https://example.com/page"));

    press(cx, web, cmd(), 1);
    release(cx, web, cmd(), 1);
    cx.run_until_parked();
    assert_eq!(*opened.borrow(), vec!["https://example.com/page".to_string()], "opened at once");
    assert!(!cx.has_pending_prompt());
    assert_eq!(ws.read_with(cx, |ws, _| ws.highlighted_text(&id)), None, "a cmd+click selects nothing");

    cx.simulate_mouse_move(file, None::<MouseButton>, cmd());
    press(cx, file, cmd(), 1);
    release(cx, file, cmd(), 1);
    cx.run_until_parked();
    assert!(cx.has_pending_prompt(), "a file:// link asks first");
    assert_eq!(opened.borrow().len(), 1, "and isn't opened yet");
    cx.simulate_prompt_answer("Open");
    cx.run_until_parked();
    assert_eq!(opened.borrow().last().map(String::as_str), Some("file:///etc/hosts"));
    s.app.stop();
}
