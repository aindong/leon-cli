//! A pane joining another session (`session.move_block` across sessions) tells both sessions'
//! subscribers, as a kill and a create would: the session it leaves sends `block_closed`, the one
//! it joins `block_created`, and each its `session_view_changed`.

mod support;

use serde_json::json;
use support::{Env, Stream};

fn until_block(s: &mut Stream, name: &str, block: &str) -> String {
    for _ in 0..50 {
        let e = s.next();
        if e.name == name && e.payload["block_id"] == block {
            return e.session_id.unwrap_or_default();
        }
    }
    panic!("no {name} for {block}");
}

#[test]
fn a_join_tells_the_session_it_leaves_and_the_one_it_joins() {
    let env = Env::new();
    env.ok(&["server", "start"]);
    let home = env.ok(&["new", "home"]);
    let anchor = env.json(&["-s", &home, "api", "call", "session.list_blocks"])["blocks"][0]["block_id"]
        .as_str()
        .unwrap()
        .to_string();
    let away = env.ok(&["new", "away"]);
    let pane = env.ok(&["-s", &away, "run", "--label", "pane", "--", "sleep", "300"]);
    let mut left = env.events(&["-s", &away]);
    let mut joined = env.events(&["-s", &home]);

    let p = json!({ "session_id": home, "block_id": pane, "anchor_block_id": anchor }).to_string();
    env.ok(&["api", "call", "session.move_block", &p]);
    assert_eq!(until_block(&mut left, "block_closed", &pane), away, "away's subscriber hears the pane leave");
    left.until("session_view_changed");
    assert_eq!(until_block(&mut joined, "block_created", &pane), home, "home's subscriber hears it arrive");
    joined.until("session_view_changed");
}
