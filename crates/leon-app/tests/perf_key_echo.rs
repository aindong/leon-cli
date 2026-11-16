//! Finding 1 of the performance baseline: a key's echo is drawn and shown at once, not at the
//! display link's next tick; once per key, so output with no key behind it waits for the tick.
mod m10b_support;

use gpui::TestAppContext;
use m10b_support::*;

#[gpui::test]
fn a_typed_key_shows_its_echo_at_once_and_output_alone_does_not(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let t = focused(&s.app, &ws, cx);
    settle(&t);
    cx.run_until_parked();
    let before = ws.read_with(cx, |ws, _| ws.echo_shows());

    // output with no key behind it: drawn on the display link
    run_until(&t, "echo out-$((1+1))", "out-2");
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.polls().1) > 0));
    settle(&t);
    cx.run_until_parked();
    assert_eq!(ws.read_with(cx, |ws, _| ws.echo_shows()), before, "no key, nothing shown at once");

    // a key: its echo's redraw is shown at once, and only that one
    cx.simulate_keystrokes("x");
    assert!(eventually(|| t.text(false).contains('x')), "the key reached the shell");
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.echo_shows()) == before + 1), "the echo was shown at once");
    cx.simulate_keystrokes("backspace");
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.echo_shows()) == before + 2), "the second key's echo too");
    settle(&t);
    cx.run_until_parked();
    let after = ws.read_with(cx, |ws, _| ws.echo_shows());
    s.app.stop();
    assert_eq!(after, before + 2, "one per key");
}

#[gpui::test]
fn with_two_windows_only_the_one_typed_in_shows_at_once(cx: &mut TestAppContext, other: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let (ws2, other) = open(other, &s.app); // the same server: both show the same pane
    let t = focused(&s.app, &ws, cx);
    settle(&t);
    cx.run_until_parked();
    other.run_until_parked();
    let (mine, theirs) = (ws.read_with(cx, |ws, _| ws.echo_shows()), ws2.read_with(other, |ws, _| ws.echo_shows()));
    let redraws = ws2.read_with(other, |ws, _| ws.polls().1);

    cx.simulate_keystrokes("y");
    assert!(eventually(|| t.text(false).contains('y')), "the key reached the shell");
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.echo_shows()) == mine + 1), "the window typed in showed it");
    assert!(shows(other, |cx| ws2.read_with(cx, |ws, _| ws.polls().1) > redraws), "the other window redrew too");
    let theirs_now = ws2.read_with(other, |ws, _| ws.echo_shows());
    s.app.stop();
    assert_eq!(theirs_now, theirs, "on its display link");
}
