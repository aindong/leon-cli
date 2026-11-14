//! M17 review (correctness): a host with 0 rows must not panic the client.

mod support;

use support::host::Host;
use support::Env;

#[test]
fn a_host_with_zero_rows_does_not_panic() {
    let env = Env::new();
    env.ok(&["run", "--", "sh"]);
    let session = env.json(&["ls", "--json"])[0]["session_id"].as_str().unwrap().to_string();
    let script =
        format!("stty rows 0 cols 80; \"$LEON\" attach {session}; echo \"after-attach rc=$?\"; exec sleep 300");
    let h = Host::spawn(&env, &script, &[]);
    h.wait_for("attached or exited", |s| h.alt_screen() || s.contains("after-attach"));
    h.send(b"\x02d");
    let s = h.wait_text("after-attach rc=");
    assert!(s.contains("after-attach rc=0"), "leon attach should detach cleanly:\n{s}");
}
