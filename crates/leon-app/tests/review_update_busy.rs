//! Review finding (.agent/plans/2026-11-16-app-update-notice.md, criteria 2 and 8): a request made
//! the moment a job's answer shows was dropped, because the job was still marked busy until after
//! its wake. No network: the install can't be found, so each look fails at once.

use std::sync::Arc;
use std::time::{Duration, Instant};

use leon_app::updates::Updater;

#[test]
fn a_check_asked_for_once_the_answer_shows_runs() {
    let wake = Arc::new(|| std::thread::sleep(Duration::from_millis(300)));
    let find = Arc::new(|| {
        std::thread::sleep(Duration::from_millis(200));
        Err::<leon_update::Target, String>("no leon here".into())
    });
    let u = Updater::new(None, find, wake);
    u.check_now();
    let end = Instant::now() + Duration::from_secs(10);
    while !u.notice().is_some_and(|n| n.text.contains("failed")) {
        assert!(Instant::now() < end, "no answer");
        std::thread::sleep(Duration::from_millis(10));
    }
    u.check_now();
    let text = u.notice().unwrap().text;
    assert!(text.contains("Looking"), "the second Check for Updates was dropped: {text}");
}

#[test]
fn a_job_that_panics_leaves_the_updater_usable() {
    let find = Arc::new(|| -> Result<leon_update::Target, String> { panic!("a bug in a job") });
    let u = Updater::new(None, find, Arc::new(|| {}));
    u.check_now();
    let end = Instant::now() + Duration::from_secs(10);
    while u.notice().is_some_and(|n| n.text.contains("Looking")) {
        assert!(Instant::now() < end, "stuck on Looking… after the job panicked");
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(u.notice().is_some_and(|n| !n.actions.is_empty()), "the notice can be dismissed: {:?}", u.notice());
}
