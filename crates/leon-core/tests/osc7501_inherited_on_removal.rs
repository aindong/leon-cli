//! Removing an ancestor that supplied `app` changes what reads return for its descendants, with only
//! the removal as an event.

use leon_core::program_status::{parse, Change, Reason, Record, State, Store, MAX_RECORDS};

fn apply(s: &mut Store, payload: &str) -> Vec<Change> {
    s.apply(parse(payload.as_bytes()).unwrap())
}

fn idle() -> Record {
    Record { state: State::Idle, kind: None, progress: None, app: None, title: None, msg: None }
}

#[test]
fn prompt_removing_the_app_ancestor() {
    let mut s = Store::default();
    apply(&mut s, "state=working:app=claude");
    apply(&mut s, "state=idle:id=a");
    assert_eq!(s.get(Some("a")).unwrap().app.as_deref(), Some("claude"));
    assert_eq!(s.prompt(), vec![Change::Removed { ids: vec![None], reason: Reason::Prompt }]);
    assert_eq!(s.get(Some("a")), Some(idle()));
}

#[test]
fn evicting_the_app_ancestor() {
    let mut s = Store::default();
    apply(&mut s, "state=idle:id=p:app=claude");
    apply(&mut s, "state=idle:id=p/c");
    for i in 2..MAX_RECORDS {
        apply(&mut s, &format!("state=idle:id=r{i}"));
    }
    assert_eq!(
        apply(&mut s, "state=idle:id=new"),
        vec![
            Change::Removed { ids: vec![Some("p".into())], reason: Reason::Evicted },
            Change::Changed { id: Some("new".into()), record: idle(), reason: Reason::Report },
        ]
    );
    assert_eq!(s.get(Some("p/c")), Some(idle()));
}
