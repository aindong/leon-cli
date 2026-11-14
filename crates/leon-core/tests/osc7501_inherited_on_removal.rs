//! Removing an ancestor that supplied `app` is a change for the descendants that survive it (on settle).

use leon_core::program_status::{parse, Change, Reason, Record, State, Store, MAX_RECORDS};

fn apply(s: &mut Store, payload: &str) -> Vec<Change> {
    s.apply(parse(payload.as_bytes()).unwrap())
}

fn idle() -> Record {
    Record { state: State::Idle, kind: None, progress: None, app: None, title: None, msg: None }
}

#[test]
fn prompt_removing_the_app_ancestor_updates_survivors() {
    let mut s = Store::default();
    apply(&mut s, "state=working:app=claude");
    apply(&mut s, "state=idle:id=a");
    let mut changes = s.prompt();
    changes.extend(s.settle());
    assert_eq!(
        changes,
        vec![
            Change::Removed { ids: vec![None], reason: Reason::Prompt },
            Change::Changed { id: Some("a".into()), record: idle(), reason: Reason::Inherited },
        ]
    );
}

#[test]
fn evicting_the_app_ancestor_updates_survivors() {
    let mut s = Store::default();
    apply(&mut s, "state=idle:id=p:app=claude");
    apply(&mut s, "state=idle:id=p/c");
    for i in 2..MAX_RECORDS {
        apply(&mut s, &format!("state=idle:id=r{i}"));
    }
    let mut changes = apply(&mut s, "state=idle:id=new");
    changes.extend(s.settle());
    assert_eq!(
        changes,
        vec![
            Change::Removed { ids: vec![Some("p".into())], reason: Reason::Evicted },
            Change::Changed { id: Some("new".into()), record: idle(), reason: Reason::Report },
            Change::Changed { id: Some("p/c".into()), record: idle(), reason: Reason::Inherited },
        ]
    );
}
