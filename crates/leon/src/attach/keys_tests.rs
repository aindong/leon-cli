use super::*;
use serde_json::json;

fn key(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, mods)
}

fn ch(c: char) -> KeyEvent {
    key(KeyCode::Char(c), KeyModifiers::NONE)
}

#[test]
fn keys_become_the_bytes_send_key_uses() {
    let ctrl = KeyModifiers::CONTROL;
    assert_eq!(bytes(&ch('a'), false), Some(b"a".to_vec()));
    assert_eq!(bytes(&key(KeyCode::Char('A'), KeyModifiers::SHIFT), false), Some(b"A".to_vec()));
    assert_eq!(bytes(&ch('é'), false), Some("é".as_bytes().to_vec()));
    assert_eq!(bytes(&key(KeyCode::Char('b'), ctrl), false), Some(vec![2]));
    assert_eq!(bytes(&key(KeyCode::Char('4'), ctrl), false), Some(vec![0x1c]), "ctrl+\\ arrives as ctrl+4");
    assert_eq!(bytes(&key(KeyCode::Char(' '), ctrl), false), Some(vec![0]), "ctrl+space is NUL");
    assert_eq!(bytes(&key(KeyCode::Char(' '), ctrl | KeyModifiers::ALT), false), Some(vec![0x1b, 0]));
    assert_eq!(bytes(&key(KeyCode::Char('x'), KeyModifiers::ALT), false), Some(b"\x1bx".to_vec()));
    assert_eq!(bytes(&key(KeyCode::Enter, KeyModifiers::NONE), false), Some(b"\r".to_vec()));
    assert_eq!(bytes(&key(KeyCode::BackTab, KeyModifiers::SHIFT), false), Some(b"\x1b[Z".to_vec()));
    assert_eq!(bytes(&key(KeyCode::F(5), KeyModifiers::NONE), false), Some(b"\x1b[15~".to_vec()));
    // arrows follow the pane's cursor-key mode, not the host's
    let up = key(KeyCode::Up, KeyModifiers::NONE);
    assert_eq!(bytes(&up, false), Some(b"\x1b[A".to_vec()));
    assert_eq!(bytes(&up, true), Some(b"\x1bOA".to_vec()));
    assert_eq!(bytes(&key(KeyCode::Left, ctrl), true), Some(b"\x1b[1;5D".to_vec()));
}

#[test]
fn keys_become_unshifted_chords() {
    assert_eq!(chord(&ch('%')), Some(one_chord("shift+5").unwrap()));
    assert_eq!(chord(&ch('"')), Some(one_chord("shift+'").unwrap()));
    assert_eq!(chord(&key(KeyCode::Char('B'), KeyModifiers::SHIFT)), Some(one_chord("shift+b").unwrap()));
    assert_eq!(chord(&key(KeyCode::Char('b'), KeyModifiers::CONTROL)), Some(one_chord("ctrl+b").unwrap()));
    assert_eq!(chord(&key(KeyCode::Left, KeyModifiers::NONE)), Some(one_chord("left").unwrap()));
    assert_eq!(chord(&ch(' ')), Some(one_chord("space").unwrap()));
    assert_eq!(one_chord("%").unwrap(), one_chord("shift+5").unwrap(), "config keys are unshifted the same way");
    assert!(one_chord("ctrl+a>b").is_err());
}

#[test]
fn after_the_prefix_the_config_comes_first() {
    let km = json!({
        "bindings": [
            { "key": "x", "mode": "attach", "action": "client.detach" },
            { "key": "ctrl+b>c", "action": "seq_it" },
        ],
        "unbinds": [{ "key": "z", "mode": "attach" }],
        "modes": [],
    });
    let k = Keys::new(&km, None).unwrap();
    assert_eq!(k.prefix, one_chord("ctrl+b").unwrap());
    let run = |a: &str| After::Run(a.to_string(), None);
    assert_eq!(k.after_prefix(&one_chord("x").unwrap()), run("client.detach"));
    assert_eq!(k.after_prefix(&one_chord("d").unwrap()), run("client.detach"));
    assert_eq!(k.after_prefix(&one_chord("%").unwrap()), run("pane.split.right"));
    assert!(
        matches!(k.after_prefix(&one_chord("c").unwrap()), After::Config(Pressed::Run(b)) if b.action.as_deref() == Some("seq_it"))
    );
    assert_eq!(k.after_prefix(&one_chord("z").unwrap()), After::Nothing, "unbound by the config");
    assert_eq!(k.after_prefix(&one_chord("ctrl+b").unwrap()), After::Literal);

    let km = json!({ "modes": [{ "name": "attach", "prefix": "ctrl+a" }] });
    assert_eq!(Keys::new(&km, None).unwrap().prefix, one_chord("ctrl+a").unwrap());
    assert_eq!(Keys::new(&km, Some("alt+space")).unwrap().prefix_name, "alt+space", "the flag wins");
    assert!(Keys::new(&km, Some("ctrl+a>b")).is_err());
}
