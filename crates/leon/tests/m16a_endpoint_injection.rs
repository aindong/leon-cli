//! M16a criterion 1 (.agent/plans/2026-11-15-m16a-remote-ssh.md): an endpoint can't smuggle ssh
//! options or shell into the ssh command line. Each bad one is a bad command line (exit 2), and
//! ssh never runs.

mod support;

use serde_json::json;
use support::remote::Remote;

#[test]
fn bad_endpoints_are_refused_before_ssh_runs() {
    let r = Remote::new();
    let marker = r.fake.path().join("pwned");
    let proxy = format!("-oProxyCommand=touch {}", marker.display());
    let bad = [
        proxy.as_str(),
        "ssh://-oProxyCommand=x",
        "ssh://a b",
        "ssh://a;b",
        "ssh://a$(id)",
        "ssh://a`id`",
        "ssh://u@-x",
        "ssh://-u@x",
        "ssh://host:0",
        "ssh://host:99999",
        "ssh://host:22x",
        "ssh://host/sock",
        "ssh://",
        "ssh://\"a\"",
        "ssh://a'b",
        "ssh://a\nb",
        "ssh://a|b",
        "ssh://a&b",
        "ssh://ü",
    ];
    for ep in bad {
        let out = r.run(&[&format!("--host={ep}"), "ls"]);
        assert_eq!(out.status.code(), Some(2), "-H {ep:?}: {}", String::from_utf8_lossy(&out.stderr));
        let out = r.run(&["hosts", "add", "a", "--", ep]);
        assert_eq!(out.status.code(), Some(2), "hosts add {ep:?}: {}", String::from_utf8_lossy(&out.stderr));
    }
    assert!(r.ssh_calls().is_empty(), "ssh ran: {:?}", r.ssh_calls());
    assert!(!marker.exists());
    assert_eq!(r.json(&["hosts", "list", "--json"]), json!([]));
}
