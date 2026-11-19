//! GPUI phase 6 (ADR R9): the link card opened for the keyboard (a right-click on a link) and a
//! row's menu take the focus; Escape or choosing an item gives it back to the pane, and typed
//! text reaches it.
mod focus_support;

use focus_support::*;
use gpui::{Modifiers, MouseButton, TestAppContext};

const LINKS: &str = "printf 'go https://example.com/page here\\n'; exec /bin/sh";

#[gpui::test]
fn the_link_card_and_a_row_menu_give_the_pane_its_keys_back(cx: &mut TestAppContext) {
    let s = plain_setup();
    let (ws, cx) = open(cx, &s.app);
    let id = tab_running(&s, &ws, cx, LINKS, "go https://example.com/page here");
    ws.update(cx, |ws, _| ws.set_opener(Box::new(|_| {})));
    let row = row_of(&ws, cx, &id, "go https://example.com/page here");
    let web = cell(&ws, cx, &id, row, 8);
    let card = |cx: &mut gpui::VisualTestContext| ws.read_with(cx, |ws, _| ws.link_card().is_some());
    let right_click_link = |cx: &mut gpui::VisualTestContext| {
        cx.simulate_mouse_down(web, MouseButton::Right, Modifiers::none());
        cx.simulate_mouse_up(web, MouseButton::Right, Modifiers::none());
        cx.run_until_parked();
    };

    right_click_link(cx);
    assert!(shows(cx, card), "the card opened");
    assert_eq!(owner(&ws, cx), Some("popover"), "the card has the focus");
    cx.simulate_keystrokes("escape");
    assert!(!card(cx), "Escape closed it");
    back_on_pane(&s, &ws, cx, "esc1").unwrap();

    right_click_link(cx);
    assert!(shows(cx, card), "the card opened again");
    cx.simulate_keystrokes("enter"); // Copy, the first item
    assert!(!card(cx), "choosing an item closed it");
    back_on_pane(&s, &ws, cx, "copy2").unwrap();

    let menu = |cx: &mut gpui::VisualTestContext| ws.read_with(cx, |ws, _| ws.row_menu().is_some());
    let p = center(at(cx, "session-0").expect("the session label"));
    right_click(cx, p);
    assert!(menu(cx), "the row's menu opened");
    assert_eq!(owner(&ws, cx), Some("popover"), "the menu has the focus");
    cx.simulate_keystrokes("escape");
    let r = back_on_pane(&s, &ws, cx, "menu3");
    s.app.stop();
    r.unwrap();
}
