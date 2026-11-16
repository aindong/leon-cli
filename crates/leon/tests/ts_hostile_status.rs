//! Tailscale criterion 3 (.agent/plans/2026-11-16-tailscale-hosts.md): the status is another
//! program's output. A name that wouldn't pass the endpoint or label check is refused, junk and
//! a flood fail in one line, and a client that never answers is given up on.

mod support;

use std::process::Stdio;
use std::time::{Duration, Instant};

use support::{tailscale::Fake, Env};

fn run(env: &Env, fake: &Fake, args: &[&str]) -> (Option<i32>, String) {
    let mut c = env.cmd();
    let out = fake.apply(&mut c).args(args).stdin(Stdio::null()).output().unwrap();
    let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(!text.contains("panicked"), "{text}");
    (out.status.code(), text)
}

#[test]
fn odd_names_are_refused() {
    let env = Env::new();
    let fake = Fake::new("odd.json");
    for name in [
        "bad name.example-tailnet.ts.net",
        &format!("{}.example-tailnet.ts.net", "a".repeat(70)),
        "semi;colon.example-tailnet.ts.net",
        "box.t@evil.example",
    ] {
        let (code, text) = run(&env, &fake, &["hosts", "add", "--tailscale", name]);
        assert!(code.is_some_and(|c| c != 0), "{name}: {text}");
        assert!(text.contains("bad "), "the parser's reason: {text}");
    }
    // by number from the list, too
    for n in 1..=6 {
        let mut c = env.cmd();
        let mut child = fake
            .apply(&mut c)
            .args(["hosts", "add", "--tailscale"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        use std::io::Write;
        child.stdin.take().unwrap().write_all(format!("{n}\n").as_bytes()).unwrap();
        let out = child.wait_with_output().unwrap();
        let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
        assert!(!text.contains('\u{1b}') && !text.contains('\u{202e}'), "the status's control text is not printed");
        assert!(!text.contains("panicked"), "{text}");
    }
    // the plain one of them is fine: its odd OS text and wrong-typed fields don't stop it
    let hosts = env.json(&["hosts", "list", "--json"]);
    let hosts = hosts.as_array().unwrap();
    assert_eq!(hosts.len(), 1, "only the plain name is added: {hosts:?}");
    assert_eq!(hosts[0]["endpoints"][0], "ssh://weird.example-tailnet.ts.net");
    fake.only_read();
}

#[test]
fn junk_and_floods_fail_in_one_line() {
    let env = Env::new();
    let fake = Fake::new("running.json");
    for junk in [
        "not json".to_string(),
        r#"{"BackendState":"Running","Peer":[1,2,3]}"#.to_string(),
        r#"{"BackendState":7}"#.to_string(),
        format!(r#"{{"BackendState":"Running","Peer":{{"k":{{"DNSName":"{}"}}}}}}"#, "x".repeat(9 << 20)),
    ] {
        fake.set("status", &junk);
        let (code, text) = run(&env, &fake, &["hosts", "add", "--tailscale", "devbox"]);
        assert_eq!(code, Some(1), "{text}");
        assert_eq!(text.trim().lines().count(), 1, "one line: {text}");
    }
    assert!(env.json(&["hosts", "list", "--json"]).as_array().unwrap().is_empty());
    fake.only_read();
}

#[test]
fn a_client_that_never_answers_or_never_stops_is_given_up_on() {
    let env = Env::new();
    for mode in ["hang", "flood"] {
        let fake = Fake::new("running.json");
        fake.set(mode, "");
        let t = Instant::now();
        let (code, text) = run(&env, &fake, &["hosts", "add", "--tailscale", "devbox"]);
        assert_eq!(code, Some(1), "{mode}: {text}");
        assert_eq!(text.trim().lines().count(), 1, "{mode}: one line: {text}");
        assert!(t.elapsed() < Duration::from_secs(8), "{mode}: took {:?}", t.elapsed());
        fake.only_read();
    }
}
