//! M8 acceptance (.agent/plans/2026-11-14-m8-persist-speed.md, criteria 2 and 3): `server stop
//! --discard` and `server start --restore=false` start empty.

mod support;

use serde_json::json;
use support::Env;

fn labels(env: &Env) -> Vec<String> {
    let ls = env.json(&["ls", "--json"]);
    ls.as_array().unwrap().iter().map(|s| s["label"].as_str().unwrap_or("").to_string()).collect()
}

#[test]
fn discard_starts_empty_and_stays_empty() {
    let env = Env::new();
    env.ok(&["server", "start"]);
    env.ok(&["new", "keep"]);
    env.ok(&["server", "stop"]);
    env.ok(&["server", "start"]);
    assert_eq!(labels(&env), ["keep"], "a plain stop and start restores");

    env.ok(&["server", "stop", "--discard"]);
    env.ok(&["server", "start"]);
    assert_eq!(env.json(&["ls", "--json"]), json!([]), "--discard: the next start is empty");
    env.ok(&["server", "stop"]);
    env.ok(&["server", "start"]);
    assert_eq!(env.json(&["ls", "--json"]), json!([]), "the discarded sessions don't come back later either");
}

#[test]
fn restore_false_starts_empty() {
    let env = Env::new();
    env.ok(&["server", "start"]);
    env.ok(&["new", "keep"]);
    env.ok(&["server", "stop"]);
    env.ok(&["server", "start", "--restore=false"]);
    assert_eq!(env.json(&["ls", "--json"]), json!([]), "--restore=false starts empty");
}
