//! Daily-driver A6: GPUI rebuilds a key's text from its key code, so an event with key code 0
//! (what automation and some accessibility tools send with their own text) arrived as `a`. The
//! event's own characters win when they disagree; every real key on code 0 stays as GPUI made it.
//! The last test drives one through `key_down`, the native event being a seam the test sets.

mod m9_support;

use gpui::{Keystroke, Modifiers, TestAppContext};
use leon_app::input::retext;
use m9_support::*;

fn ks(key: &str, key_char: Option<&str>, modifiers: Modifiers) -> Keystroke {
    Keystroke { modifiers, key: key.into(), key_char: key_char.map(String::from) }
}

#[test]
fn a_synthetic_x_on_key_code_0_is_an_x() {
    let got = retext(ks("a", Some("a"), Modifiers::none()), 0, "x");
    assert_eq!((got.key.as_str(), got.key_char.as_deref()), ("x", Some("x")));
    let got = retext(ks("a", Some("a"), Modifiers::none()), 0, "hello");
    assert_eq!(got.key_char.as_deref(), Some("hello"), "a whole string typed in one event");
    assert_eq!(leon_app::input::key_bytes(&got, false), Some(b"hello".to_vec()));
}

#[test]
fn real_keys_on_key_code_0_are_unchanged() {
    let shift = Modifiers { shift: true, ..Modifiers::none() };
    let cases = [
        (ks("a", Some("a"), Modifiers::none()), "a"),   // QWERTY a
        (ks("a", Some("A"), shift), "A"),               // shift-a
        (ks("a", Some("å"), Modifiers::alt()), "å"),    // option-a
        (ks("a", None, Modifiers::control()), "\u{1}"), // ctrl-a
        (ks("a", None, Modifiers::command()), "a"),     // cmd-a
        (ks("q", Some("q"), Modifiers::none()), "q"),   // AZERTY: q is on code 0
        (ks("a", Some("ф"), Modifiers::none()), "ф"),   // Russian, GPUI's command layout key
        (ks("a", Some("a"), Modifiers::none()), ""),    // no characters at all
    ];
    for (k, chars) in cases {
        assert_eq!(retext(k.clone(), 0, chars), k, "{chars:?}");
    }
}

#[test]
fn other_key_codes_are_left_alone() {
    let k = ks("s", Some("s"), Modifiers::none());
    assert_eq!(retext(k.clone(), 1, "x"), k);
}

#[gpui::test]
fn a_key_code_0_event_types_its_own_text(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let t = focused(&s.app, &ws, cx);
    settle(&t);
    cx.simulate_keystrokes("e c h o space");
    ws.update(cx, |ws, _| ws.set_native_key(|| Some((0, "x".into()))));
    cx.simulate_keystrokes("a"); // GPUI's keystroke for code 0, as it makes one from the layout
    ws.update(cx, |ws, _| ws.set_native_key(|| None));
    cx.simulate_keystrokes("enter");
    assert!(eventually(|| has_line(&t, "x")), "the pane got x, not a:\n{}", t.text(false));
    s.app.stop();
}
