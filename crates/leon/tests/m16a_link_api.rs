//! M16a criterion 13 (.agent/plans/2026-11-15-m16a-remote-ssh.md): what the app (M16b) builds
//! on: the hosts list, an endpoint, and a `Link` whose socket the app's own `Remote` attaches to.

mod support;

use support::remote::Remote;

#[test]
fn the_app_can_attach_through_a_link() {
    let r = Remote::new();
    r.ok(&["-H", "ssh://box", "new", "api-1"]);
    r.ok(&["hosts", "add", "devbox", "ssh://box"]);
    // this process plays the app: it needs the fake ssh's variables and the local dirs
    for (k, v) in r.vars() {
        std::env::set_var(k, v);
    }
    std::env::set_var("LEON_RUNTIME_DIR", r.local.dir.path());
    std::env::set_var("LEON_STATE_DIR", r.local.state_dir());

    let hosts: Vec<leon_hosts::Host> = leon_hosts::hosts::load().unwrap();
    assert_eq!(hosts[0].label, "devbox");
    let ep: leon_hosts::Endpoint = hosts[0].endpoints[0].parse().unwrap();
    let link = leon_hosts::Link::open("devbox", &ep, true).unwrap();
    let remote = leon_app::remote::Remote::connect(link.socket()).unwrap();
    let model = remote.model();
    assert!(model.sessions.iter().any(|s| s.label.as_deref() == Some("api-1")), "{:?}", model.sessions);
}
