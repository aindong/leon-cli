//! M16a review: opening a link must not make another live link run ssh. The stale-dir check used
//! to connect to every link's socket, and a live link answers a connection by running ssh.

mod support;

use std::time::{Duration, Instant};

use support::remote::Remote;

fn calls_to(r: &Remote, host: &str) -> usize {
    r.ssh_calls()
        .iter()
        .filter(|argv| {
            argv.iter().position(|a| a == "--").is_some_and(|i| argv.get(i + 1).map(String::as_str) == Some(host))
        })
        .count()
}

#[test]
fn another_link_opening_does_not_make_a_live_link_ssh() {
    let r = Remote::new();
    let ev = r.events(&["-H", "ssh://box"]);
    let before = calls_to(&r, "box");
    assert!(before >= 1, "events reached box through the fake ssh");

    // a different host, so this command's own ssh never goes to box
    assert_eq!(r.code(&["-H", "ssh://unreachable-probe", "ls"]), 4);

    let end = Instant::now() + Duration::from_secs(3);
    while Instant::now() < end {
        let now = calls_to(&r, "box");
        assert_eq!(
            now, before,
            "opening a link to another host made the running `events` link ssh to box again ({before} -> {now})"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    drop(ev);
}
