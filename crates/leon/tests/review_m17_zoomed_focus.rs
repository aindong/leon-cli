//! M17 review: with a pane zoomed that isn't the focused one, prefix+arrow moves from the pane on
//! screen (as the app does), not from the hidden focused pane.

mod support;

use support::host::Host;
use support::Env;

#[test]
fn focus_moves_from_the_zoomed_pane() {
    let env = Env::new();
    let a = env.ok(&["run", "--", "sh"]);
    let session = env.json(&["ls", "--json"])[0]["session_id"].as_str().unwrap().to_string();
    let b = env.ok(&["-s", &session, "split", "-b", &a, "--", "sh"]);
    let c = env.ok(&["-s", &session, "split", "-b", &b, "--", "sh"]);
    env.ok(&["-s", &session, "zoom", &a, "--on"]);
    let w = env.window(&session, 0);
    assert_eq!((w["focused_block_id"].as_str(), w["zoomed_block_id"].as_str()), (Some(c.as_str()), Some(a.as_str())));
    let h = Host::spawn(&env, &format!("exec \"$LEON\" attach {session}"), &[]);
    h.wait_for("attached", |_| h.alt_screen() && h.last_row().contains("leon"));
    h.send(b"\x02\x1b[C"); // prefix, right
    Host::until("focus moved right from the zoomed pane", || {
        let w = env.window(&session, 0);
        w["focused_block_id"] == b.as_str() && w["zoomed_block_id"].is_null()
    });
}
