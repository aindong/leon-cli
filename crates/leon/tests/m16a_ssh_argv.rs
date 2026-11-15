//! M16a criterion 2 (.agent/plans/2026-11-15-m16a-remote-ssh.md): the exact ssh argv. Only `-T`,
//! no forwarding (added after the security review) and the keepalives are leon's; the user and host go after `--`, then one fixed remote
//! command that finds `leon` on PATH or in ~/.local/bin.

mod support;

use support::remote::Remote;

fn expect(port: Option<&str>, dest: &str) -> Vec<String> {
    let mut v: Vec<String> = [
        "-T",
        "-a",
        "-x",
        "-o",
        "ClearAllForwardings=yes",
        "-o",
        "ServerAliveInterval=15",
        "-o",
        "ServerAliveCountMax=3",
        "-o",
        // daily-driver F11: an unanswered connect or banner fails instead of hanging the command
        "ConnectTimeout=15",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    if let Some(p) = port {
        v.extend(["-p".to_string(), p.to_string()]);
    }
    v.extend(["--".to_string(), dest.to_string()]);
    v
}

#[test]
fn ssh_gets_the_endpoint_as_separate_args_after_double_dash() {
    let r = Remote::new();
    r.ok(&["-H", "ssh://me@box:2222", "ls"]);
    let calls = r.ssh_calls();
    assert!(!calls.is_empty());
    for c in &calls {
        let (remote_cmd, head) = c.split_last().unwrap();
        assert_eq!(head, expect(Some("2222"), "me@box").as_slice(), "{c:?}");
        assert!(remote_cmd.starts_with("sh -c '"), "{remote_cmd}");
        assert!(remote_cmd.contains("exec leon server proxy"), "{remote_cmd}");
        assert!(remote_cmd.contains("$HOME/.local/bin/leon"), "{remote_cmd}");
    }
    let n = calls.len();
    r.ok(&["--host", "ssh://box", "ls"]);
    let calls = r.ssh_calls();
    let (_, head) = calls[n].split_last().unwrap();
    assert_eq!(head, expect(None, "box").as_slice());
}
