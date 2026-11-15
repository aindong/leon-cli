//! Daily-driver A4: after a restart a restored block keeps the title its program had set (tabs
//! were all named after the shell), until the program running there now sets one of its own.

mod support;

use std::time::{Duration, Instant};

use support::Env;

fn title(env: &Env, b: &str) -> Option<String> {
    env.json(&["block", "inspect", b, "--json"])["title"].as_str().map(String::from)
}

fn title_becomes(env: &Env, b: &str, want: &str) {
    let end = Instant::now() + Duration::from_secs(10);
    while title(env, b).as_deref() != Some(want) {
        assert!(Instant::now() < end, "the title stayed {:?}, not {want:?}", title(env, b));
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn a_restored_block_keeps_its_title_until_the_program_sets_one() {
    let env = Env::new();
    env.ok(&["server", "start"]);
    let b = env.ok(&["run", "--", "/bin/sh"]);
    env.ok(&["-b", &b, "send", "printf '\\033]2;Fix the tests\\007'\r"]);
    title_becomes(&env, &b, "Fix the tests");
    env.ok(&["server", "stop"]);

    env.ok(&["server", "start"]);
    env.screen_until(&b, "the restored shell", |s| s.contains("— restored from "));
    assert_eq!(title(&env, &b).as_deref(), Some("Fix the tests"), "the saved title came back");
    let listed = env.json(&["block", "ls", "--json"]);
    let row = listed.as_array().unwrap().iter().find(|r| r["block_id"] == b.as_str()).expect("listed");
    assert_eq!(row["title"], "Fix the tests", "list_blocks says it too: {row}");

    env.ok(&["-b", &b, "send", "printf '\\033]2;Next step\\007'\r"]);
    title_becomes(&env, &b, "Next step");
    env.ok(&["server", "stop"]);
}
