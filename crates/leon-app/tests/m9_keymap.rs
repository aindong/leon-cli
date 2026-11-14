//! M9 criterion 4: bindings from init.lua reach the app's key path: chords, sequences with a hint,
//! config overriding a built-in shortcut, config actions run on the server, unbound keys typed.

mod m9_support;

use gpui::{Keystroke, Modifiers, TestAppContext};
use leon_app::keymap::chord;
use leon_lua::keys::parse;
use m9_support::*;

fn ks(key: &str, m: Modifiers) -> Keystroke {
    Keystroke { modifiers: m, key: key.into(), key_char: None }
}

fn one(spec: &str) -> leon_lua::keys::Chord {
    parse(spec).unwrap().seq.remove(0)
}

#[test]
fn keystrokes_become_unshifted_binding_chords() {
    let cmd = Modifiers::command();
    assert_eq!(chord(&ks("t", cmd)), Some(one("cmd+t")));
    assert_eq!(
        chord(&ks("p", Modifiers { platform: true, shift: true, ..Default::default() })),
        Some(one("cmd+shift+p"))
    );
    // macOS reports cmd+shift+= as "+" with no shift, and cmd+shift+] as "}"
    assert_eq!(chord(&ks("+", cmd)), Some(one("cmd+shift+=")));
    assert_eq!(chord(&ks("}", cmd)), Some(one("cmd+shift+]")));
    assert_eq!(chord(&ks("=", cmd)), Some(one("cmd+=")));
    assert_eq!(chord(&ks("-", cmd)), Some(one("cmd+-")));
    assert_eq!(
        chord(&ks("enter", Modifiers { platform: true, shift: true, ..Default::default() })),
        Some(one("cmd+shift+enter"))
    );
    assert_eq!(chord(&ks("b", Modifiers::control())), Some(one("ctrl+b")));
    assert_eq!(
        chord(&ks("left", Modifiers { platform: true, alt: true, ..Default::default() })),
        Some(one("cmd+alt+left"))
    );
    assert_eq!(chord(&ks("A", Modifiers::shift())), Some(one("shift+a")));
}

const CONFIG: &str = r#"
leon.bind("cmd+shift+r", "client.tab.new")
leon.bind("ctrl+b>c", "client.tab.new")
leon.bind("ctrl+b>v", "pane.split.right")
leon.bind("cmd+d", "client.tab.new")
leon.action{ name = "mark", title = "Mark", run = function(ctx) leon.session.set_label{ label = "marked" } end }
leon.bind("cmd+shift+y", "mark")
leon.unbind("cmd+shift+d")
leon.bind("cmd+t>1", "pane.split.right")
"#;

#[gpui::test]
fn lua_bindings_drive_the_app(cx: &mut TestAppContext) {
    let s = setup_with_config(CONFIG);
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.sync_keymap());
    let tabs = |cx: &mut gpui::VisualTestContext| window(&s.app, &ws, cx).1;
    assert_eq!(tabs(cx), 1);

    cx.simulate_keystrokes("cmd-shift-r");
    assert_eq!(tabs(cx), 2, "a chord bound in Lua runs its client action");

    cx.simulate_keystrokes("ctrl-b");
    let hint = ws.read_with(cx, |ws, _| ws.pending_hint()).expect("ctrl+b is a pending prefix");
    assert_eq!(hint.0, "ctrl+b");
    assert!(hint.1.iter().any(|(k, _)| k == "c") && hint.1.iter().any(|(k, _)| k == "v"), "{hint:?}");
    cx.simulate_keystrokes("c");
    assert!(ws.read_with(cx, |ws, _| ws.pending_hint()).is_none(), "the sequence completed");
    assert_eq!(tabs(cx), 3, "ctrl+b>c ran");

    cx.simulate_keystrokes("ctrl-b escape");
    assert!(ws.read_with(cx, |ws, _| ws.pending_hint()).is_none(), "escape cancels");
    assert_eq!(tabs(cx), 3);

    cx.simulate_keystrokes("cmd-d");
    assert_eq!(tabs(cx), 4, "the config's cmd+d overrides the built-in split");
    assert_eq!(leaves(&window(&s.app, &ws, cx).0).len(), 1, "and nothing split");
    cx.simulate_keystrokes("cmd-shift-d");
    assert_eq!(leaves(&window(&s.app, &ws, cx).0).len(), 1, "leon.unbind removed the built-in split down");
    cx.simulate_keystrokes("cmd-t");
    assert_eq!(tabs(cx), 4, "cmd+t is now the prefix of cmd+t>1, not new tab");
    assert_eq!(ws.read_with(cx, |ws, _| ws.pending_hint()).map(|h| h.0).as_deref(), Some("cmd+t"));
    cx.simulate_keystrokes("1");
    assert_eq!(leaves(&window(&s.app, &ws, cx).0).len(), 2, "cmd+t>1 split");
    cx.simulate_keystrokes("cmd-alt-left");
    assert_eq!(leaves(&window(&s.app, &ws, cx).0).len(), 2, "built-ins the config leaves alone still work");

    cx.simulate_keystrokes("cmd-shift-y");
    let sid = ws.read_with(cx, |ws, _| ws.session_id()).unwrap();
    let label = || s.app.ctx().state().session(&sid).unwrap().label.clone();
    assert!(eventually(|| label().as_deref() == Some("marked")), "the config action ran on the server: {:?}", label());

    let t = focused(&ws, cx);
    cx.simulate_input("echo unbound-ok");
    cx.simulate_keystrokes("enter");
    assert!(eventually(|| has_line(&t, "unbound-ok")), "unbound keys reach the pane:\n{}", t.text(true));
    s.app.stop();
}
