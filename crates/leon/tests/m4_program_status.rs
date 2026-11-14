//! M4b acceptance tests, criteria 12–14 (.agent/plans/2026-11-14-m3-layout-events.md): OSC 7501
//! reports from a real program become events and block-method records; queries get a reply.

mod support;

use support::{Env, Ev, Stream};

const WORKING: &str = r"printf '\033]7501;state=working\033\\'";

fn split(env: &Env, anchor: &str, extra: &[&str], script: &str) -> String {
    let mut args = vec!["split", "-b", anchor, "--json"];
    args.extend_from_slice(extra);
    args.extend_from_slice(&["--", "sh", "-c", script]);
    env.json(&args)["block_id"].as_str().unwrap().to_string()
}

/// The next block event of `block` named `name`.
fn block_event(ev: &mut Stream, block: &str, name: &str) -> Ev {
    loop {
        let e = ev.until(name);
        if e.block_id.as_deref() == Some(block) {
            return e;
        }
    }
}

#[test]
fn c12_reports_become_events_and_records_and_exit_drops_them() {
    let env = Env::new();
    let (s, a) = env.session();
    let mut ev = env.events(&["-s", &s]);
    let b = split(&env, &a, &["--keep-open"], &format!("{WORKING}; read x"));

    let e = block_event(&mut ev, &b, "program_status_changed");
    assert!(e.payload["id"].is_null(), "the root record: {e:?}");
    assert_eq!(e.payload["record"]["state"], "working");
    assert_eq!(e.payload["reason"], "report");

    let r = env.json(&["block", "call", "-b", &b, "program_status"]);
    let records = r["records"].as_array().unwrap_or_else(|| panic!("no records: {r}"));
    assert_eq!(records.len(), 1, "{r}");
    assert!(records[0]["id"].is_null());
    assert_eq!(records[0]["record"]["state"], "working");

    env.ok(&["send-key", "-b", &b, "enter"]);
    let mut got = vec![];
    while got.len() < 2 {
        let e = ev.next();
        assert_ne!(e.name, "block_closed", "a kept-open block closed");
        if e.block_id.as_deref() == Some(b.as_str()) {
            got.push(e);
        }
    }
    assert_eq!(got[0].name, "program_status_removed", "the removal comes before the exit: {got:?}");
    assert_eq!(got[0].payload["ids"], serde_json::json!([null]));
    assert_eq!(got[0].payload["reason"], "exited");
    assert_eq!(got[1].name, "child_exited", "{got:?}");
    assert_eq!(got[1].payload["exit_code"], 0);
    let r = env.json(&["block", "call", "-b", &b, "dev.leon.terminal.program_status"]);
    assert_eq!(r["records"], serde_json::json!([]), "exit dropped the working record");
    assert_eq!(env.json(&["block", "inspect", &b, "--json"])["exit_code"], 0, "still open");
}

#[test]
fn c13_a_new_prompt_drops_working() {
    let env = Env::new();
    let (s, a) = env.session();
    let mut ev = env.events(&["-s", &s]);
    // Each step waits for Enter, so each lands in a read of its own: within one read a report and
    // its removal net out to the removal alone (the reader coalesces each read's changes).
    let script =
        format!(r"{WORKING}; read x; printf '\033]133;A\007'; read x; {WORKING}; read x; printf '\033c'; read x");
    let b = split(&env, &a, &[], &script);
    let enter = || env.ok(&["send-key", "-b", &b, "enter"]);
    block_event(&mut ev, &b, "program_status_changed");
    enter();
    let e = block_event(&mut ev, &b, "program_status_removed");
    assert_eq!(e.payload["ids"], serde_json::json!([null]));
    assert_eq!(e.payload["reason"], "prompt");
    enter();
    block_event(&mut ev, &b, "program_status_changed");
    enter();
    let e = block_event(&mut ev, &b, "program_status_removed");
    assert_eq!(e.payload["reason"], "reset", "RIS wipes the records");
}

#[test]
fn c14_queries_get_a_reply_on_the_pty() {
    let env = Env::new();
    let (_s, a) = env.session();
    let script = r"stty raw -echo; printf '\033]7501;?\033\\'; dd bs=1 count=15 2>/dev/null | tr -d '\033'; sleep 30";
    let b = split(&env, &a, &[], script);
    env.screen_until(&b, "the query reply", |s| s.contains("]7501;?:states"));
}
