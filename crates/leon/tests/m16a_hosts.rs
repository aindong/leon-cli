//! M16a criterion 6 (.agent/plans/2026-11-15-m16a-remote-ssh.md): `leon hosts` round-trips a
//! private hosts file, `-H LABEL` uses it, and a file that isn't private, or an endpoint edited
//! into it by hand, is refused.

mod support;

use std::os::unix::fs::PermissionsExt;

use serde_json::json;
use support::remote::Remote;

fn mode(p: &std::path::Path) -> u32 {
    std::fs::metadata(p).unwrap().permissions().mode() & 0o777
}

#[test]
fn hosts_round_trip() {
    let r = Remote::new();
    assert_eq!(r.json(&["hosts", "list", "--json"]), json!([]));
    r.ok(&["hosts", "add", "devbox", "ssh://me@box:2222", "--icon", "laptop"]);
    assert_eq!(
        r.json(&["hosts", "list", "--json"]),
        json!([{ "label": "devbox", "endpoints": ["ssh://me@box:2222"], "icon": "laptop" }])
    );
    assert!(r.ok(&["hosts", "list"]).contains("devbox"));
    let file = r.local.state_dir().join("hosts.json");
    assert_eq!(mode(&file), 0o600);
    assert_eq!(mode(&r.local.state_dir()), 0o700);
    assert_eq!(r.code(&["hosts", "add", "devbox", "ssh://other"]), 1, "a label is added once");
    assert_eq!(r.code(&["hosts", "add", "--", "-x", "ssh://box"]), 2);
    assert_eq!(r.code(&["hosts", "add", "a b", "ssh://box"]), 2);

    r.ok(&["-H", "devbox", "new", "via-label"]);
    let last = r.ssh_calls().pop().unwrap();
    assert!(last.contains(&"me@box".to_string()) && last.contains(&"2222".to_string()), "{last:?}");
    assert_eq!(r.remote.json(&["--autostart=false", "ls", "--json"])[0]["label"], "via-label");

    let out = r.ok(&["hosts", "check", "devbox"]);
    assert!(out.contains("devbox") && out.contains("ok"), "{out}");
    r.ok(&["hosts", "add", "down", "ssh://unreachable.example"]);
    assert_eq!(r.code(&["hosts", "check"]), 4, "one host is down");
    assert_eq!(r.code(&["hosts", "check", "down"]), 4);

    r.ok(&["hosts", "remove", "devbox", "down"]);
    assert_eq!(r.json(&["hosts", "list", "--json"]), json!([]));
    assert_eq!(r.code(&["hosts", "remove", "devbox"]), 3);
    assert_eq!(r.code(&["-H", "devbox", "ls"]), 3);
}

#[test]
fn a_hand_edited_endpoint_is_checked_again() {
    let r = Remote::new();
    r.ok(&["hosts", "add", "evil", "ssh://box"]);
    let file = r.local.state_dir().join("hosts.json");
    let text = std::fs::read_to_string(&file).unwrap().replace("ssh://box", "-oProxyCommand=x");
    std::fs::write(&file, text).unwrap();
    assert_eq!(r.code(&["-H", "evil", "ls"]), 2);
    assert!(r.ssh_calls().is_empty());
}

#[test]
fn a_hosts_file_in_an_open_dir_is_refused() {
    let r = Remote::new();
    r.ok(&["hosts", "add", "devbox", "ssh://box"]);
    let dir = r.local.state_dir();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o770)).unwrap();
    let out = r.run(&["hosts", "list"]);
    assert_eq!(out.status.code(), Some(1), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(!String::from_utf8_lossy(&out.stdout).contains("devbox"));
    assert_eq!(r.code(&["-H", "devbox", "ls"]), 1);
    assert!(r.ssh_calls().is_empty());
    assert_eq!(mode(&dir), 0o770, "leon never chmods");
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
}
