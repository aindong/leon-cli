//! Review: no hover card over a program that asked for the mouse (vim, htop): the card would sit
//! on cells the program takes clicks on. ⌘-click still opens the link.
mod link_card_support;

use gpui::{Modifiers, TestAppContext};
use link_card_support::*;

const TRACKING: &str =
    "stty -echo -icanon; printf '\\033[?1000h\\033[?1006hgo https://example.com/page here\\n'; cat -v";

#[gpui::test]
fn resting_over_a_mouse_program_shows_no_card(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let id = tab_running(&s, &ws, cx, TRACKING, "go https://example.com/page here");
    let row = row_of(&ws, cx, &id, "go https://example.com/page here");
    let link = cell(&ws, cx, &id, row, 8);
    rest(&ws, cx, link, Modifiers::none(), 600);
    let (shown, asked) = (card(&ws, cx), asks(&ws, cx));
    s.app.stop();
    assert_eq!(shown, None, "no card");
    assert_eq!(asked, 0, "no lookup");
}
