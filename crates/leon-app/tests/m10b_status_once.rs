//! M10b criterion 5: status shows once per place. The tab row and the pane header carry it; a
//! session label only while collapsed; the caption sits in the pane header, not over the terminal.

mod m10b_support;

use gpui::TestAppContext;
use leon_app::status::Indicator;
use m10b_support::*;

// base64 of "Claude" and "thinking"
const WORKING: &str = r"printf '\033]7501;state=working:title=Q2xhdWRl:msg=dGhpbmtpbmc=\007'; echo w-sent; sleep 60";

#[gpui::test]
fn the_tab_row_and_pane_header_show_status_and_the_label_does_not(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let id = ws.read_with(cx, |ws, _| ws.target_block()).unwrap();
    run_until(&focused(&s.app, &ws, cx), WORKING, "w-sent");
    let working = Some(Indicator::Working { progress: None });
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.indicators()).1 == vec![working]));
    assert!(drawn(cx, "row-status-0-0"), "the tab row shows it");
    assert!(drawn(cx, &format!("header-status-{id}")), "the pane header shows it");
    assert!(!drawn(cx, "session-status-0"), "an expanded session's label does not");

    let caption = at(cx, &format!("caption-{id}")).expect("the caption is drawn");
    let header = at(cx, &format!("header-{id}")).expect("the pane has a header");
    assert!(header.contains(&center(caption)), "the caption is in the header: {caption:?} {header:?}");
    let term = at(cx, &format!("terminal-{id}")).expect("the terminal");
    assert!(caption.bottom() <= term.top(), "and does not cover the terminal");

    let label = center(at(cx, "session-0").unwrap());
    cx.simulate_click(label, gpui::Modifiers::none());
    assert!(!drawn(cx, "row-0-0"), "clicking the selected session's label collapses it");
    assert!(drawn(cx, "session-status-0"), "a collapsed label shows the session's status");
    s.app.stop();
}
