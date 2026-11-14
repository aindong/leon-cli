//! M13 criterion 7: `leon notify` sets its block's status through the server, so a script or a
//! tool without OSC 7501 lights up the Activity list; the record goes through the spec's limits.

mod support;

use support::{Env, Ev, Stream};

fn root(env: &Env, block: &str) -> serde_json::Value {
    let r = env.json(&["block", "call", "-b", block, "program_status"]);
    let records = r["records"].as_array().unwrap_or_else(|| panic!("no records: {r}")).clone();
    assert!(records.len() <= 1, "{r}");
    records.first().map(|r| r["record"].clone()).unwrap_or_default()
}

fn status_event(ev: &mut Stream, block: &str) -> Ev {
    loop {
        let e = ev.until("program_status_changed");
        if e.block_id.as_deref() == Some(block) {
            return e;
        }
    }
}

#[test]
fn notify_sets_the_blocks_state_title_and_body() {
    let env = Env::new();
    let (s, b) = env.session();
    let mut ev = env.events(&["-s", &s]);
    env.ok(&["notify", "-b", &b, "--state", "blocked", "--title", "Deploy", "--body", "approve the rollout?"]);
    let e = status_event(&mut ev, &b);
    assert_eq!(e.payload["record"]["state"], "blocked", "{e:?}");
    let r = root(&env, &b);
    assert_eq!(r["state"], "blocked");
    assert_eq!(r["title"], "Deploy");
    assert_eq!(r["msg"], "approve the rollout?");

    for state in ["working", "error", "done"] {
        env.ok(&["notify", "-b", &b, "--state", state]);
        assert_eq!(root(&env, &b)["state"], state, "--state {state}");
    }
    env.ok(&["notify", "-b", &b, "--state", "working", "--title", "Build"]);
    env.ok(&["notify", "-b", &b, "--title", "Built"]);
    let r = root(&env, &b);
    assert_eq!((r["state"].as_str(), r["title"].as_str()), (Some("done"), Some("Built")), "the default state is done");
}

#[test]
fn notify_finds_its_own_block_from_the_environment() {
    let env = Env::new();
    let (s, b) = env.session();
    let out = env.cmd().env("LEON_SESSION", &s).env("LEON_BLOCK", &b).args(["notify", "--state", "error"]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(root(&env, &b)["state"], "error");
}

#[test]
fn notify_without_a_block_refuses_rather_than_guess() {
    let env = Env::new();
    let (_, b) = env.session();
    let out = env.run(&["notify", "--state", "blocked"]);
    assert_eq!(out.status.code(), Some(2), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stderr).contains("LEON_BLOCK"), "says how to name the block");
    assert!(root(&env, &b).is_null(), "no block got a record");
}

#[test]
fn notify_keeps_to_the_specs_limits() {
    let env = Env::new();
    let (_, b) = env.session();
    let long = "t".repeat(193); // titles are at most 192 bytes decoded
    let out = env.run(&["notify", "-b", &b, "--title", &long]);
    assert!(!out.status.success(), "an over-long title is refused");
    assert!(root(&env, &b).is_null(), "and nothing was recorded");
    let out = env.run(&["notify", "-b", &b, "--state", "sleeping"]);
    assert_eq!(out.status.code(), Some(2), "an unknown state is a bad command line");
    for bad in ["a\u{202e}b", "bell\u{7}", "two\nlines"] {
        let out = env.run(&["notify", "-b", &b, "--state", "blocked", "--title", bad]);
        assert!(!out.status.success(), "{bad:?}: text OSC 7501 would drop is refused, not dropped silently");
        let out = env.run(&["notify", "-b", &b, "--state", "blocked", "--body", bad]);
        assert!(!out.status.success(), "{bad:?} in the body too");
    }
    assert!(root(&env, &b).is_null(), "nothing was recorded");
}
