//! Each tab, sidebar row, overview card and pane header shows the glyph of its program's kind: a
//! tab's is the pane it shows (the focused one), and it follows the focus. The status mark keeps
//! its place: after the glyph and the title in a header, a tab and a row.

mod m10b_support;

use gpui::TestAppContext;
use m10b_support::*;

#[gpui::test]
fn every_place_shows_the_shown_panes_kind(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let shell = ws.read_with(cx, |ws, _| ws.target_block()).expect("a pane");
    act(&ws, cx, "pane.split.right");
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.target_block().is_some_and(|t| t != shell))), "no split");
    let editor = ws.read_with(cx, |ws, _| ws.target_block()).unwrap();
    // the shell reports working (OSC 7501) and is back at its prompt: a shell glyph with a status mark
    let working = r"printf '\033]7501;state=working\007'; echo sent";
    run_until(&term(&s.app, &shell), working, "sent");
    term(&s.app, &editor).write(b"vi -u NONE -N\r").unwrap();

    let both = [format!("header-program-{shell}-shell"), format!("header-program-{editor}-editor")];
    assert!(shows(cx, |cx| both.iter().all(|h| drawn(cx, h))), "each pane's header shows its own kind");
    assert!(drawn(cx, "row-program-0-0-editor"), "the tab's sidebar row shows the focused pane's");

    // the status mark keeps its place: after the glyph and the title
    let glyph = at(cx, &format!("header-program-{shell}-shell")).unwrap();
    let title = at(cx, &format!("header-title-{shell}")).unwrap();
    let status = at(cx, &format!("header-status-{shell}")).expect("the header still shows the status");
    assert!(glyph.right() <= title.left() && title.right() <= status.left(), "{glyph:?} {title:?} {status:?}");

    act(&ws, cx, "pane.focus.left");
    assert!(shows(cx, |cx| drawn(cx, "row-program-0-0-shell")), "the row follows the focus");
    let row = at(cx, "row-program-0-0-shell").unwrap();
    let mark = at(cx, "row-status-0-0").expect("the row still shows the status");
    assert!(row.right() <= mark.left(), "the row's status mark stays after the glyph: {row:?} {mark:?}");

    act(&ws, cx, "client.tab_style.toggle");
    assert!(shows(cx, |cx| drawn(cx, "tab-program-0-shell")), "a top tab shows it");
    let tab = at(cx, "tab-program-0-shell").unwrap();
    let mark = at(cx, "tab-status-0").expect("the tab still shows the status");
    assert!(tab.right() <= mark.left(), "the tab's status mark stays after the glyph: {tab:?} {mark:?}");
    act(&ws, cx, "pane.focus.right");
    assert!(shows(cx, |cx| drawn(cx, "tab-program-0-editor")), "and follows the focus");

    cx.simulate_keystrokes("cmd-shift-\\");
    let wid = ws.read_with(cx, |ws, _| ws.overview_cards().first().map(|c| c.window_id.clone())).expect("a card");
    assert!(shows(cx, |cx| drawn(cx, &format!("card-program-{wid}-editor"))), "the card shows it too");
    let card_status = at(cx, &format!("card-status-{wid}")).expect("the card still shows the status");
    let card_glyph = at(cx, &format!("card-program-{wid}-editor")).unwrap();
    assert!(card_status.right() <= card_glyph.left(), "a card's status mark stays first");
    s.app.stop();
}
