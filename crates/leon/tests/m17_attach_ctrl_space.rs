//! M17 review (skeptic): ctrl+space on the host must reach the pane as NUL (0x00), the byte
//! `leon send-key ctrl+space` sends.

mod support;

use support::host::{has_words, Host};
use support::Env;

#[test]
fn ctrl_space_types_nul_into_the_pane() {
    let env = Env::new();
    env.ok(&["run", "--", "sh"]);
    let session = env.json(&["ls", "--json"])[0]["session_id"].as_str().unwrap().to_string();
    let h = Host::spawn(&env, &format!("exec \"$LEON\" attach {session}"), &[]);
    h.wait_for("attached", |_| h.alt_screen() && h.last_row().contains("leon"));

    // od dumps what the pane reads: NUL (ctrl+space as the host sends it), then the newline
    h.send(b"od -An -tx1\r");
    h.send(b"\x00\r\x04");
    h.wait_for("od shows the NUL byte from ctrl+space", |s| has_words(s, &["00", "0a"]));
}
