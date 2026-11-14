//! M17 review (correctness): a prefix on 0x1c–0x1f (ctrl+\ ] ^ _), which crossterm reports as
//! ctrl+4…7, still works as the prefix.

mod support;

use support::host::{assert_restored, attach_then_stty, Host};
use support::Env;

#[test]
fn a_ctrl_bracket_prefix_detaches() {
    let env = Env::new();
    env.ok(&["run", "--", "sh"]);
    let session = env.json(&["ls", "--json"])[0]["session_id"].as_str().unwrap().to_string();
    let h = Host::spawn(&env, &attach_then_stty(&format!("--prefix 'ctrl+]' {session}")), &[]);
    h.wait_for("attached", |_| h.alt_screen() && h.last_row().contains("leon"));
    h.send(b"\x1dd"); // ctrl+] (0x1d) then d
    assert_restored(&h, 0);
}
