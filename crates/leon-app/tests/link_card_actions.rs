//! The link card's Copy puts the address on the clipboard; Open in Browser opens it through the
//! app's opener (at once for a web link, after asking for anything else, as ⌘-click does).
mod link_card_support;

use gpui::{Modifiers, TestAppContext};
use link_card_support::*;

#[gpui::test]
fn copy_puts_the_address_on_the_clipboard(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let l = links(&s, &ws, cx);
    rest(&ws, cx, l.web, Modifiers::none(), 500);
    assert!(card(&ws, cx).is_some(), "the card shows");
    click_item(cx, "popover-item-copy");
    assert_eq!(clipboard(cx).as_deref(), Some(WEB));
    assert_eq!(card(&ws, cx), None, "the card closes");
    assert!(cx.update(|window, cx| ws.read(cx).has_focus(window)), "and gives the keyboard back to the panes");
    assert!(l.opened.borrow().is_empty(), "nothing opened");
    s.app.stop();
}

#[gpui::test]
fn open_goes_through_the_opener(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let l = links(&s, &ws, cx);
    rest(&ws, cx, l.web, Modifiers::none(), 500);
    click_item(cx, "popover-item-open");
    assert_eq!(*l.opened.borrow(), vec![WEB.to_string()], "opened at once");
    assert!(!cx.has_pending_prompt());
    assert_eq!(card(&ws, cx), None, "the card closes");

    rest(&ws, cx, l.file, Modifiers::none(), 500);
    assert_eq!(card(&ws, cx).and_then(|l| l.first().cloned()).as_deref(), Some("file:///etc/hosts"));
    click_item(cx, "popover-item-open");
    assert!(cx.has_pending_prompt(), "a file:// link asks first");
    assert_eq!(l.opened.borrow().len(), 1, "and isn't opened yet");
    cx.simulate_prompt_answer("Open");
    cx.run_until_parked();
    assert_eq!(l.opened.borrow().last().map(String::as_str), Some("file:///etc/hosts"));
    s.app.stop();
}
