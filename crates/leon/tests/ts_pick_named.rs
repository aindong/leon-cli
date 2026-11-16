//! Tailscale criterion 2 (.agent/plans/2026-11-16-tailscale-hosts.md): a machine named on the
//! command line is added without a prompt, and one without Tailscale SSH says what to do.

mod support;

use std::process::Stdio;

use support::{tailscale::Fake, Env};

#[test]
fn a_named_machine_is_added_and_one_without_tailscale_ssh_says_what_to_do() {
    let env = Env::new();
    let fake = Fake::new("running.json");
    let run = |args: &[&str]| {
        let mut c = env.cmd();
        fake.apply(&mut c).args(args).stdin(Stdio::null()).output().unwrap()
    };

    let out = run(&["hosts", "add", "--tailscale", "laptop"]);
    let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(out.status.success(), "{text}");
    assert!(text.contains("Tailscale SSH"), "says it has no Tailscale SSH: {text}");
    assert!(text.contains("tailscale set --ssh"), "how to turn it on there: {text}");
    assert!(text.contains("ssh key"), "or the other way in: {text}");
    assert!(text.contains("leon hosts check laptop"), "check it once in a terminal: {text}");

    // the MagicDNS name works too, and one with Tailscale SSH needs no advice
    let out = run(&["hosts", "add", "--tailscale", "devbox.example-tailnet.ts.net", "--icon", "server"]);
    let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(out.status.success(), "{text}");
    assert!(!text.contains("tailscale set --ssh"), "{text}");

    let hosts = env.json(&["hosts", "list", "--json"]);
    assert_eq!(hosts[0]["label"], "laptop");
    assert_eq!(hosts[0]["endpoints"][0], "ssh://laptop.example-tailnet.ts.net");
    assert_eq!(hosts[1]["label"], "devbox");
    assert_eq!(hosts[1]["icon"], "server");

    // a machine that isn't on the tailnet: not found, nothing written
    let out = run(&["hosts", "add", "--tailscale", "nosuchbox"]);
    assert_eq!(out.status.code(), Some(3), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(env.json(&["hosts", "list", "--json"]).as_array().unwrap().len(), 2);

    // a label in use is refused as `hosts add` refuses it
    assert_eq!(run(&["hosts", "add", "--tailscale", "laptop"]).status.code(), Some(1));
    // endpoints don't go with --tailscale
    assert_eq!(run(&["hosts", "add", "--tailscale", "x", "ssh://x"]).status.code(), Some(2));

    // a short name two machines share (a machine shared in from another tailnet) is ambiguous
    let running = std::fs::read_to_string(support::tailscale::fixture("running.json")).unwrap();
    let shared = running.replace(
        r#""nodekey:cc": {"#,
        r#""nodekey:dd": { "DNSName": "twin.other-tailnet.ts.net.", "OS": "linux", "Online": true },
    "nodekey:ee": { "DNSName": "twin.example-tailnet.ts.net.", "OS": "linux", "Online": true },
    "nodekey:cc": {"#,
    );
    fake.set("status", &shared);
    let out = run(&["hosts", "add", "--tailscale", "twin"]);
    let text = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success() && text.contains("MagicDNS name"), "{text}");
    assert_eq!(env.json(&["hosts", "list", "--json"]).as_array().unwrap().len(), 2, "nothing written");
    assert!(run(&["hosts", "add", "--tailscale", "twin.other-tailnet.ts.net"]).status.success());

    // MagicDNS off: the name wouldn't resolve, so the endpoint is the machine's tailnet address
    let off = running.replacen(
        r#""MagicDNSSuffix": "example-tailnet.ts.net","#,
        r#""MagicDNSSuffix": "example-tailnet.ts.net", "CurrentTailnet": { "MagicDNSEnabled": false },"#,
        1,
    );
    fake.set("status", &off);
    assert!(run(&["hosts", "add", "--tailscale", "buildbox"]).status.success());
    let hosts = env.json(&["hosts", "list", "--json"]);
    let last = hosts.as_array().unwrap().last().unwrap().clone();
    assert_eq!((last["label"].as_str(), last["endpoints"][0].as_str()), (Some("buildbox"), Some("ssh://100.64.0.4")));
    fake.only_read();
}
