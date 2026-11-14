//! Record store: replace, clear, app inheritance, LRU cap, lifetime hooks, change events.

use leon_core::program_status::{parse, Change, Kind, Message, Reason, Record, State, Store, MAX_RECORDS};

fn msg(payload: &str) -> Message {
    parse(payload.as_bytes()).unwrap_or_else(|e| panic!("{payload:?} was ignored: {e:?}"))
}

fn rec(state: State) -> Record {
    Record { state, kind: None, progress: None, app: None, title: None, msg: None }
}

fn id(s: &str) -> Option<String> {
    Some(s.to_string())
}

fn removed(ids: &[Option<String>], reason: Reason) -> Vec<Change> {
    vec![Change::Removed { ids: ids.to_vec(), reason }]
}

#[test]
fn report_replaces_its_record_and_emits_changed() {
    let mut s = Store::default();
    let blocked = Record { kind: Some(Kind::Question), progress: Some(5), ..rec(State::Blocked) };
    assert_eq!(
        s.apply(msg("state=blocked:kind=question:progress=5")),
        vec![Change::Changed { id: None, record: blocked.clone(), reason: Reason::Report }]
    );
    assert_eq!(s.get(None), Some(blocked));
    assert_eq!(s.apply(msg("state=blocked:kind=question:progress=5")), vec![], "identical report is not a change");
    assert_eq!(
        s.apply(msg("state=idle")),
        vec![Change::Changed { id: None, record: rec(State::Idle), reason: Reason::Report }]
    );
    assert_eq!(s.get(None), Some(rec(State::Idle)), "kind and progress don't survive a replace");
    assert_eq!(s.apply(Message::Query), vec![]);
    assert_eq!(s.len(), 1);
}

#[test]
fn clear_removes_record_and_descendants() {
    let mut s = Store::default();
    for i in ["a", "a/b", "a/b/c", "ab", "x"] {
        s.apply(msg(&format!("state=working:id={i}")));
    }
    assert_eq!(s.apply(msg("state=clear:id=a")), removed(&[id("a"), id("a/b"), id("a/b/c")], Reason::Clear));
    assert_eq!(s.get(Some("ab")), Some(rec(State::Working)), "ab is not a child of a");
    assert_eq!(s.apply(msg("state=clear:id=nope")), vec![]);
    s.apply(msg("state=done"));
    s.apply(msg("state=idle:id=q/r"));
    assert_eq!(s.apply(msg("state=clear:id=q")), removed(&[id("q/r")], Reason::Clear), "missing parent, child cleared");
    assert_eq!(s.apply(msg("state=clear")), removed(&[None, id("ab"), id("x")], Reason::Clear));
    assert_eq!(s.len(), 0);
    assert_eq!(s.apply(msg("state=clear")), vec![]);
}

#[test]
fn app_is_inherited_from_the_nearest_ancestor() {
    let mut s = Store::default();
    s.apply(msg("state=working:id=x"));
    assert_eq!(s.get(Some("x")).unwrap().app, None);
    s.apply(msg("state=working:app=claude"));
    assert_eq!(
        s.apply(msg("state=working:id=a")),
        vec![Change::Changed {
            id: id("a"),
            record: Record { app: Some("claude".into()), ..rec(State::Working) },
            reason: Reason::Report
        }]
    );
    s.apply(msg("state=working:id=a/b:app=sub"));
    s.apply(msg("state=working:id=a/b/c"));
    s.apply(msg("state=working:id=a/z/c"));
    assert_eq!(s.get(Some("a/b/c")).unwrap().app.as_deref(), Some("sub"));
    assert_eq!(s.get(Some("a/z/c")).unwrap().app.as_deref(), Some("claude"), "skips the missing a/z");
    assert_eq!(s.get(Some("x")).unwrap().app.as_deref(), Some("claude"), "resolved when read");
    let listed = s.records();
    assert_eq!(listed.len(), 6, "x, root, a, a/b, a/b/c, a/z/c");
    assert!(listed.iter().all(|(_, r)| r.app.is_some()), "records() resolves app too: {listed:?}");
}

#[test]
fn descendants_get_an_inherited_event_when_their_app_changes() {
    let mut s = Store::default();
    s.apply(msg("state=working:id=a"));
    s.apply(msg("state=working:id=a/b:app=own"));
    s.apply(msg("state=working:id=a/c"));
    let claude = |state| Record { app: Some("claude".into()), ..rec(state) };
    assert_eq!(
        s.apply(msg("state=idle:app=claude")),
        vec![
            Change::Changed { id: None, record: claude(State::Idle), reason: Reason::Report },
            Change::Changed { id: id("a"), record: claude(State::Working), reason: Reason::Inherited },
            Change::Changed { id: id("a/c"), record: claude(State::Working), reason: Reason::Inherited },
        ],
        "a/b keeps its own app, so it gets no event"
    );
    assert_eq!(s.apply(msg("state=done:app=claude")).len(), 1, "same app, only the root changes");
    assert_eq!(s.prompt(), removed(&[id("a"), id("a/b"), id("a/c")], Reason::Prompt));
    s.apply(msg("state=working:id=a/c"));
    assert_eq!(
        s.apply(msg("state=clear")),
        vec![Change::Removed { ids: vec![None, id("a/c")], reason: Reason::Clear }],
        "removed records get no inherited event"
    );
    s.apply(msg("state=idle:app=claude"));
    s.apply(msg("state=working:id=k"));
    assert_eq!(
        s.apply(msg("state=idle")),
        vec![
            Change::Changed { id: None, record: rec(State::Idle), reason: Reason::Report },
            Change::Changed { id: id("k"), record: rec(State::Working), reason: Reason::Inherited },
        ],
        "losing the inherited app is a change too"
    );
}

#[test]
fn lru_eviction_at_256_records() {
    assert_eq!(MAX_RECORDS, 256);
    let mut s = Store::default();
    for i in 0..MAX_RECORDS {
        s.apply(msg(&format!("state=working:id=r{i}")));
    }
    assert_eq!(s.len(), MAX_RECORDS);
    s.apply(msg("state=done:id=r0")); // r0 is now the most recently reported
    assert_eq!(
        s.apply(msg("state=working:id=new")),
        vec![
            Change::Removed { ids: vec![id("r1")], reason: Reason::Evicted },
            Change::Changed { id: id("new"), record: rec(State::Working), reason: Reason::Report },
        ]
    );
    assert_eq!(s.len(), MAX_RECORDS);
    assert_eq!(s.get(Some("r0")), Some(rec(State::Done)));
    assert_eq!(s.get(Some("r1")), None);
    // replacing an existing record never evicts
    assert!(s.apply(msg("state=idle:id=r2")).iter().all(|c| matches!(c, Change::Changed { .. })));
}

#[test]
fn lifetime_hooks() {
    let fill = |s: &mut Store| {
        for p in ["state=working:id=a", "state=blocked:id=b", "state=idle:id=c", "state=done:id=d", "state=error:id=e"]
        {
            s.apply(msg(p));
        }
    };
    let mut s = Store::default();
    fill(&mut s);
    assert_eq!(s.process_exited(), removed(&[id("a"), id("b")], Reason::Exited));
    assert_eq!(s.len(), 3);
    assert_eq!(s.process_exited(), vec![], "nothing left to drop, no event");
    fill(&mut s);
    assert_eq!(s.prompt(), removed(&[id("a"), id("b")], Reason::Prompt));
    assert_eq!(s.reset(), removed(&[id("c"), id("d"), id("e")], Reason::Reset));
    assert_eq!(s.reset(), vec![]);
}

#[test]
fn events_serialize_with_spec_names() {
    let mut s = Store::default();
    s.apply(msg("state=working:app=x"));
    let changes = s.apply(msg("state=blocked:id=a:kind=question:progress=5"));
    assert_eq!(
        serde_json::to_value(&changes[0]).unwrap(),
        serde_json::json!({
            "type": "program_status_changed",
            "id": "a",
            "record": {"state": "blocked", "kind": "question", "progress": 5, "app": "x"},
            "reason": "report"
        })
    );
    assert_eq!(
        serde_json::to_value(&s.reset()[0]).unwrap(),
        serde_json::json!({"type": "program_status_removed", "ids": [null, "a"], "reason": "reset"})
    );
}
