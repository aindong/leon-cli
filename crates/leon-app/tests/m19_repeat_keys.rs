//! M19 criterion 2 (.agent/plans/2026-11-15-m19-tmux-migrants.md): the repeat state machine
//! (tmux `bind -r`), on an injected clock.

use std::time::{Duration, Instant};

use leon_app::keymap::{Keymap, Pressed, Repeat};
use leon_lua::keys::{parse, Chord};
use serde_json::json;

fn one(s: &str) -> Chord {
    parse(s).unwrap().seq.remove(0)
}

fn keymap(repeat_time: u64) -> Keymap {
    Keymap::new(&json!({
        "bindings": [
            { "key": "ctrl+b>h", "action": "pane.focus.left", "repeats": true },
            { "key": "ctrl+b>l", "action": "pane.focus.right", "repeats": true },
            { "key": "ctrl+b>c", "action": "client.tab.new" },
            { "key": "ctrl+a>h", "action": "pane.zoom", "repeats": true },
            { "key": "ctrl+y", "action": "pane.zoom", "repeats": true },
        ],
        "settings": { "repeat_time": repeat_time },
    }))
}

/// Run `key` after `prefix` as the app does: complete a binding and tell the repeat it ran.
fn run(km: &Keymap, r: &mut Repeat, prefix: &[Chord], key: &str, now: Instant) -> Option<String> {
    match km.press(prefix, &one(key)) {
        Pressed::Run(b) => {
            r.ran(prefix, b.repeats, km.repeat_time, now);
            b.action
        }
        _ => None,
    }
}

/// A key while the window may be open: the repeatable action it runs again, if any.
fn again(km: &Keymap, r: &mut Repeat, key: &str, now: Instant) -> Option<String> {
    let (b, prefix) = r.press(&one(key), now, |p, k| km.repeatable(p, k))?;
    r.ran(&prefix, b.repeats, km.repeat_time, now);
    b.action
}

#[test]
fn the_window_reruns_restarts_and_switches_between_repeatable_keys() {
    let km = keymap(300);
    assert_eq!(km.repeat_time, Duration::from_millis(300));
    assert_eq!(Keymap::new(&json!({})).repeat_time, Duration::from_millis(500), "the default");
    let (mut r, t0, ms) = (Repeat::default(), Instant::now(), Duration::from_millis);
    let b = [one("ctrl+b")];
    assert_eq!(km.press(&[], &b[0]), Pressed::Pending);
    assert_eq!(run(&km, &mut r, &b, "h", t0).as_deref(), Some("pane.focus.left"));
    assert_eq!(again(&km, &mut r, "h", t0 + ms(200)).as_deref(), Some("pane.focus.left"), "no prefix");
    // the repeat at 200 ms opened a new window: 450 ms is inside it
    assert_eq!(again(&km, &mut r, "l", t0 + ms(450)).as_deref(), Some("pane.focus.right"), "another repeatable");
    assert_eq!(again(&km, &mut r, "c", t0 + ms(500)), None, "c doesn't repeat: it ends the window");
    assert_eq!(again(&km, &mut r, "h", t0 + ms(510)), None, "and the window stays closed");
}

#[test]
fn a_late_key_or_a_plain_binding_opens_nothing() {
    let km = keymap(300);
    let (mut r, t0, ms) = (Repeat::default(), Instant::now(), Duration::from_millis);
    let b = [one("ctrl+b")];
    run(&km, &mut r, &b, "h", t0);
    assert_eq!(again(&km, &mut r, "h", t0 + ms(301)), None, "past the window");
    run(&km, &mut r, &b, "c", t0);
    assert_eq!(again(&km, &mut r, "h", t0 + ms(10)), None, "c has no repeats");
    run(&km, &mut r, &[], "ctrl+y", t0);
    assert_eq!(again(&km, &mut r, "ctrl+y", t0 + ms(10)), None, "a one-key binding has no prefix to skip");
    run(&km, &mut r, &b, "h", t0);
    assert_eq!(again(&km, &mut r, "x", t0 + ms(10)), None, "an unbound key ends it");
    run(&km, &mut r, &[one("ctrl+a")], "h", t0);
    assert_eq!(again(&km, &mut r, "l", t0 + ms(10)), None, "l repeats after ctrl+b, not after ctrl+a");
}

#[test]
fn repeat_time_zero_turns_it_off() {
    let km = keymap(0);
    let (mut r, t0) = (Repeat::default(), Instant::now());
    run(&km, &mut r, &[one("ctrl+b")], "h", t0);
    assert_eq!(again(&km, &mut r, "h", t0), None);
}
