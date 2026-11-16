//! A font change fits only the panes on screen: a pane of a session the app doesn't show keeps its
//! size (its history is not reallocated at a new width) until it is shown, and is fitted then.
mod m9_support;
use gpui::TestAppContext;
use m9_support::*;

#[gpui::test]
fn a_hidden_pane_keeps_its_size_until_it_is_shown(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let shown = focused(&s.app, &ws, cx);
    let here = ws.read_with(cx, |ws, _| ws.session_id()).unwrap();
    let (there, block) = hidden_session(&s.app, &ws, cx);
    let hidden = term(&s.app, &block);
    ws.update(cx, |ws, cx| ws.select(&there, None, cx));
    assert!(shows(cx, |_| hidden.size() != (24, 80)), "the pane was fitted when shown");
    let at_default = hidden.size();
    ws.update(cx, |ws, cx| ws.select(&here, None, cx));
    cx.run_until_parked();
    let before = shown.size();

    cx.simulate_keystrokes("cmd-=");
    assert!(shows(cx, |_| shown.size() != before), "the shown pane follows the font");
    let kept = stays(cx, &hidden, at_default);
    ws.update(cx, |ws, cx| ws.select(&there, None, cx));
    let fitted = shows(cx, |_| hidden.size() != at_default);
    let got = hidden.size();
    s.app.stop();
    assert!(kept, "a hidden pane keeps its size through a font change");
    assert!(
        fitted && got.0 < at_default.0 && got.1 < at_default.1,
        "shown, it fits the bigger font: {at_default:?} -> {got:?}"
    );
}
