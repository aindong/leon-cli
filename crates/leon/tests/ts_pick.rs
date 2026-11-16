//! Tailscale criterion 1 (.agent/plans/2026-11-16-tailscale-hosts.md): `leon hosts add --tailscale`
//! lists the tailnet's machines from the local client's status and adds the one picked as an
//! ordinary host entry. leon only reads the status.

mod support;

use std::io::Write;
use std::process::Stdio;

use serde_json::json;
use support::{tailscale::Fake, Env};

#[test]
fn picks_a_machine_from_the_tailnet() {
    let env = Env::new();
    let fake = Fake::new("running.json");
    let mut c = env.cmd();
    fake.apply(&mut c).args(["hosts", "add", "--tailscale"]);
    let mut child = c.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    child.stdin.take().unwrap().write_all(b"1\n").unwrap();
    let out = child.wait_with_output().unwrap();
    let (stdout, stderr) = (String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(out.status.success(), "{stdout}\n{stderr}");

    // every peer, not this machine, online ones first, each with its name, OS and marks
    let at = |s: &str| stdout.find(s).unwrap_or_else(|| panic!("{s:?} not listed:\n{stdout}"));
    assert!(at("buildbox.example-tailnet.ts.net") < at("laptop.example-tailnet.ts.net"), "online first:\n{stdout}");
    assert!(at("devbox.example-tailnet.ts.net") < at("laptop.example-tailnet.ts.net"));
    assert!(!stdout.contains("mymac"), "not this machine:\n{stdout}");
    let line = |name: &str| stdout.lines().find(|l| l.contains(name)).unwrap().to_string();
    assert!(line("devbox.").contains("linux") && line("devbox.").contains("Tailscale SSH"), "{stdout}");
    assert!(!line("buildbox.").contains("Tailscale SSH"), "{stdout}");
    assert!(line("laptop.").contains("offline") && line("laptop.").contains("macOS"), "{stdout}");
    assert!(line("devbox.").contains("online"), "{stdout}");
    assert!(stdout.contains("me@example.com"), "who you are on the tailnet:\n{stdout}");

    // row 1 (online, then by name: buildbox) was added as a plain host: label from the machine,
    // endpoint its MagicDNS name
    let first = stdout.lines().find(|l| l.trim_start().starts_with("1 ")).expect("numbered rows").to_string();
    assert!(first.contains("buildbox."), "{stdout}");
    let label = "buildbox";
    assert_eq!(
        env.json(&["hosts", "list", "--json"])
            .as_array()
            .unwrap()
            .iter()
            .map(|h| json!({"label": h["label"], "endpoints": h["endpoints"]}))
            .collect::<Vec<_>>(),
        vec![json!({ "label": label, "endpoints": [format!("ssh://{label}.example-tailnet.ts.net")] })]
    );
    let file = std::fs::read_to_string(env.state_dir().join("hosts.json")).unwrap();
    assert!(!file.to_lowercase().contains("tailscale") && !file.contains("tailnet\""), "no new kind of host: {file}");

    assert!(!fake.argv().is_empty());
    fake.only_read();
}
