//! M16b impact: over a link a watchdog pings a quiet event connection and ends it when no line
//! comes back. A healthy link that stays idle past two ping periods must keep its stream: the
//! server answers the ping on an attached connection, so nothing is resumed or re-attached.

mod support;

use std::time::Duration;

use support::remote::Remote;

#[test]
fn an_idle_link_is_not_mistaken_for_a_dead_one() {
    let r = Remote::new();
    r.remote.ok(&["new", "idle"]);
    for (k, v) in r.vars() {
        std::env::set_var(k, v);
    }
    std::env::set_var("LEON_RUNTIME_DIR", r.local.dir.path());
    let ep: leon_hosts::Endpoint = "ssh://box".parse().unwrap();
    let link = leon_hosts::Link::open("box", &ep, true).unwrap();
    let remote = leon_app::remote::Remote::connect_retrying(link.socket(), Duration::from_secs(60)).unwrap();
    let before = r.ssh_calls().len();
    std::thread::sleep(Duration::from_secs(13)); // two ping periods and more, with nothing happening
    assert_eq!((remote.resumes(), remote.reattaches()), (0, 0), "the idle stream was kept");
    assert_eq!(r.ssh_calls().len(), before, "and no new ssh ran");
    assert!(remote.alive());
}
