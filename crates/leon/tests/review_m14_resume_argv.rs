//! Review fix (M14): a resume command is typed at a shell prompt under "ask", so what the user sees
//! must be what runs. A backslash (fish reads `\'` inside single quotes) and bidi, invisible or
//! non-ASCII characters (a line that looks like another; readline's convert-meta) are refused.

mod support;

use serde_json::{json, Value};
use support::Env;

fn refused(env: &Env, b: &str, command: Value) -> bool {
    let args = json!({ "command": command }).to_string();
    let o = env.run(&["-b", b, "block", "call", "set_resume", &args]);
    o.status.code() == Some(1) && String::from_utf8_lossy(&o.stderr).contains("bad_request")
}

#[test]
fn a_backslash_is_refused_until_quoting_is_per_shell() {
    let env = Env::new();
    env.ok(&["server", "start"]);
    let b = env.ok(&["run", "--", "/bin/sh"]);
    assert!(refused(&env, &b, json!(["tool", "a\\", "b c"])), "backslash accepted: fish reads it inside single quotes");
}

#[test]
fn bidi_and_invisible_characters_are_refused() {
    let env = Env::new();
    env.ok(&["server", "start"]);
    let b = env.ok(&["run", "--", "/bin/sh"]);
    for arg in ["\u{202e}x", "a\u{200b}b", "a\u{2028}b", "\u{e9}"] {
        assert!(refused(&env, &b, json!(["tool", arg])), "accepted {arg:?}: the typed line can look like another");
    }
}
