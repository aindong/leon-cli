//! M17 review (correctness): the host shrinking between layout and drawing must not panic the
//! client on a buffer index (a window drag sends a stream of SIGWINCH).

mod support;

use support::host::Host;
use support::Env;

#[test]
fn shrinking_the_host_mid_draw_never_panics() {
    let env = Env::new();
    env.ok(&["run", "--", "sh"]);
    let session = env.json(&["ls", "--json"])[0]["session_id"].as_str().unwrap().to_string();
    let script = format!("\"$LEON\" attach {session} 2>&1; echo \"after-attach rc=$?\"; exec sleep 300");
    let h = Host::spawn(&env, &script, &[]);
    h.wait_for("attached", |_| h.alt_screen() && h.last_row().contains("leon"));
    h.send(b"\x02%");
    h.wait_for("two panes", |s| s.contains('│'));
    h.send(b"\x02\"");
    h.wait_for("three panes", |s| s.contains('─'));
    // each draw resizes every pane (round trips) between reading the size and drawing
    for i in 0..300 {
        let (rows, cols) = if i % 2 == 0 { (60, 200) } else { (6, 12) };
        h.term.resize(rows, cols).unwrap();
        if h.screen().contains("after-attach") {
            break;
        }
    }
    h.term.resize(24, 80).unwrap();
    let s = h.wait_for("a settled draw or an exit", |s| {
        s.contains("after-attach") || (s.lines().count() == 24 && s.lines().last().unwrap_or("").contains("leon"))
    });
    assert!(!s.contains("after-attach"), "leon attach died:\n{s}");
}
