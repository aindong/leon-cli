//! Review: a call that fails changes nothing, so it must bump no revision and emit no event
//! (.agent/plans/2026-11-14-m3-layout-events.md: "A no-op ... bumps nothing"; creation is all or nothing).

mod support;

use support::{revision, Env};

#[test]
fn a_failed_run_leaves_revision_and_stream_untouched() {
    let env = Env::new();
    let (s, _a) = env.session();
    let before = revision(&env.view(&s));
    let mut ev = env.events(&["-s", &s]);

    let out = env.run(&["-s", &s, "run", "-c", "/nonexistent-leon-review-dir", "--", "true"]);
    assert!(!out.status.success(), "run in a missing directory must fail");

    assert_eq!(revision(&env.view(&s)), before, "a failed call bumped the revision");
    env.ok(&["session", "rename", &s, "after"]);
    let first = ev.next();
    assert_eq!(first.name, "session_label_changed", "a failed call emitted {first:?}");
}

#[test]
fn a_failed_session_create_says_nothing_on_the_lifecycle_stream() {
    let env = Env::new();
    env.ok(&["server", "start"]);
    let mut life = env.events(&[]);
    assert!(!env.run(&["run", "-c", "/nonexistent-leon-review-dir", "--", "true"]).status.success());
    let x = env.ok(&["new", "x"]);
    let e = life.next();
    assert_eq!((e.name.as_str(), e.session_id.as_deref()), ("session_created", Some(x.as_str())), "{e:?}");
}
