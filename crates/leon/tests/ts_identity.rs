//! Tailscale criterion 5 (.agent/plans/2026-11-16-tailscale-hosts.md): `leon hosts` shows which
//! hosts are tailnet machines and who you reach them as; the others print as before.

mod support;

use support::{tailscale::Fake, Env};

#[test]
fn tailnet_hosts_show_who_you_reach_them_as() {
    let env = Env::new();
    let fake = Fake::new("running.json");
    env.ok(&["hosts", "add", "devbox", "ssh://devbox.example-tailnet.ts.net"]);
    env.ok(&["hosts", "add", "build", "ssh://me@BUILDBOX.example-tailnet.ts.net."]);
    env.ok(&["hosts", "add", "plain", "ssh://box.example.com"]);
    let run = |args: &[&str]| {
        let mut c = env.cmd();
        let out = fake.apply(&mut c).args(args).output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8(out.stdout).unwrap()
    };

    let text = run(&["hosts", "list"]);
    let line = |l: &str| text.lines().find(|x| x.starts_with(l)).unwrap_or_else(|| panic!("{l}: {text}")).to_string();
    assert!(line("devbox").contains("tailnet: me@example.com"), "{text}");
    assert!(!line("devbox").contains("no Tailscale SSH"), "{text}");
    assert!(line("build").contains("tailnet: me@example.com") && line("build").contains("no Tailscale SSH"), "{text}");
    assert_eq!(line("plain"), "plain  ssh://box.example.com", "{text}");

    let json: serde_json::Value = serde_json::from_str(&run(&["hosts", "list", "--json"])).unwrap();
    assert_eq!(
        json[0]["tailnet"],
        serde_json::json!({ "login": "me@example.com", "online": true, "ssh": true }),
        "{json}"
    );
    assert_eq!(json[1]["tailnet"]["ssh"], false);
    assert!(json[2].get("tailnet").is_none(), "{json}");
    fake.only_read();
}
