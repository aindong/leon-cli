//! M17 criterion 9: over a slow link a keystroke must not cost a whole screen. One echoed
//! character costs the host a few cells, not a redraw (24×80 is 1920 cells before any styling).

mod support;

use std::time::{Duration, Instant};

use support::host::Host;
use support::Env;

/// Wait until the host's byte count stops moving for `quiet`.
fn settle(h: &Host, quiet: Duration) -> u64 {
    let end = Instant::now() + Duration::from_secs(10);
    let mut last = h.output_bytes();
    let mut since = Instant::now();
    while Instant::now() < end {
        std::thread::sleep(Duration::from_millis(20));
        let now = h.output_bytes();
        if now != last {
            (last, since) = (now, Instant::now());
        } else if since.elapsed() >= quiet {
            return now;
        }
    }
    panic!("the host never went quiet");
}

#[test]
fn one_keystroke_costs_a_few_cells() {
    let env = Env::new();
    env.ok(&["run", "--", "sh"]);
    let session = env.json(&["ls", "--json"])[0]["session_id"].as_str().unwrap().to_string();
    let h = Host::spawn(&env, &format!("exec \"$LEON\" attach {session}"), &[("COLORTERM", "truecolor")]);
    h.wait_for("attached", |_| h.alt_screen() && h.last_row().contains("leon"));
    let before = settle(&h, Duration::from_millis(300));
    h.send(b"q");
    h.wait_for("the echoed q", |s| s.lines().next().is_some_and(|l| l.trim_end().ends_with('q')));
    let after = settle(&h, Duration::from_millis(300));
    assert!(after - before < 400, "one echoed key cost {} bytes on the host", after - before);
}

/// A flood draws the newest screen, not every screen: the host gets far fewer bytes than the
/// program wrote (one draw per server update would cost more than the output itself).
#[test]
fn a_flood_is_coalesced() {
    let env = Env::new();
    env.ok(&["run", "--", "sh"]);
    let session = env.json(&["ls", "--json"])[0]["session_id"].as_str().unwrap().to_string();
    let h = Host::spawn(&env, &format!("exec \"$LEON\" attach {session}"), &[("COLORTERM", "truecolor")]);
    h.wait_for("attached", |_| h.alt_screen() && h.last_row().contains("leon"));
    let before = settle(&h, Duration::from_millis(300));
    // `seq 1 200000` writes 1,288,895 bytes
    h.send(b"seq 1 200000; echo flood-$((1+1))\r");
    h.wait_text("flood-2");
    let cost = settle(&h, Duration::from_millis(300)) - before;
    assert!(cost < 1_288_895 / 2, "the flood cost {cost} bytes on the host");
}
