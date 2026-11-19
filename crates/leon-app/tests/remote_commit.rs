//! GPUI phase 5 (ADR R4): the window's copy of a machine's model moves only when the main thread
//! commits it, and the commit says what moved. A `Remote` never committed (`leon attach`) reads
//! the model the reader thread keeps.
mod m9_support;

use leon_app::remote::Remote;
use m9_support::*;
use serde_json::json;

fn name(r: &Remote, sid: &str) -> Option<String> {
    r.model().session(sid).and_then(|s| s.label.clone())
}

#[test]
fn the_copy_moves_only_on_commit() {
    let s = setup();
    let sid = s.app.call("session.create", json!({})).unwrap()["session_id"].as_str().unwrap().to_string();
    let r = Remote::connect(s.app.socket()).unwrap();
    let first = r.commit();
    assert!(r.model().session(&sid).is_some(), "the first commit has the model: {first:?}");

    s.app.call("session.set_label", json!({ "session_id": sid, "label": "renamed" })).unwrap();
    // the reader refetched: its own model, which the window never draws from, has the label
    let live = |r: &Remote| r.live().session(&sid).and_then(|s| s.label.clone());
    assert!(eventually(|| live(&r).as_deref() == Some("renamed")), "the reader refetched the renamed session");
    assert_eq!(name(&r, &sid), None, "the copy is the old one until the main thread commits");

    let c = r.commit();
    assert!(c.sessions, "the commit says the sessions moved: {c:?}");
    assert_eq!(name(&r, &sid).as_deref(), Some("renamed"));
}

#[test]
fn a_pushed_screen_is_reported_by_its_block() {
    let s = setup();
    let created = s.app.call("session.create", json!({})).unwrap();
    let sid = created["session_id"].as_str().unwrap().to_string();
    let r = Remote::connect(s.app.socket()).unwrap();
    let block = r.model().session(&sid).unwrap().blocks[0].id.clone();
    r.commit();
    r.watch(vec![block.clone()]);
    let mut seen = Vec::new();
    assert!(
        eventually(|| {
            seen.extend(r.commit().screens);
            seen.contains(&block)
        }),
        "the watched block's screen came and the commit named it"
    );
    assert!(r.model().screens.contains_key(&block), "the copy has its screen");
}

#[test]
fn a_remote_never_committed_reads_the_live_model() {
    let s = setup();
    let sid = s.app.call("session.create", json!({})).unwrap()["session_id"].as_str().unwrap().to_string();
    let r = Remote::connect(s.app.socket()).unwrap();
    s.app.call("session.set_label", json!({ "session_id": sid, "label": "live" })).unwrap();
    assert!(eventually(|| name(&r, &sid).as_deref() == Some("live")), "attach sees the reader's model");
}
