//! A right-click on a link opens the link card at once, ready for the keyboard; a program that
//! asked for mouse reports still gets the right-click, and no card opens.
mod link_card_support;

use gpui::TestAppContext;
use link_card_support::*;

#[gpui::test]
fn a_right_click_opens_the_card_for_the_keyboard(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let l = links(&s, &ws, cx);
    right_click(cx, l.web);
    let lines = card(&ws, cx).expect("the card opens at once");
    assert_eq!(lines.first().map(String::as_str), Some(WEB));
    assert_eq!(ws.read_with(cx, |ws, cx| ws.link_card().and_then(|p| p.read(cx).highlighted())), Some(0));
    cx.simulate_keystrokes("enter");
    assert_eq!(clipboard(cx).as_deref(), Some(WEB), "Enter chose Copy");
    assert_eq!(card(&ws, cx), None);

    right_click(cx, l.off);
    assert_eq!(card(&ws, cx), None, "off a link a right-click opens nothing");
    s.app.stop();
}

/// `cat -v` shows what the program reads: ESC as `^[`.
const TRACKING: &str =
    "stty -echo -icanon; printf '\\033[?1000h\\033[?1006hgo https://example.com/page here\\n'; cat -v";

#[gpui::test]
fn a_program_that_asked_for_the_mouse_gets_the_right_click(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let id = tab_running(&s, &ws, cx, TRACKING, "go https://example.com/page here");
    let t = term(&s.app, &id);
    let row = row_of(&ws, cx, &id, "go https://example.com/page here");
    let link = cell(&ws, cx, &id, row, 8);
    right_click(cx, link);
    assert_eq!(card(&ws, cx), None, "no card");
    let want = format!("^[[<2;9;{}M", row + 1);
    assert!(eventually(|| t.text(true).contains(&want)), "the right press as SGR {want}: {}", t.text(true));
    s.app.stop();
}
