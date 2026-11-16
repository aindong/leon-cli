//! App update criterion 2 (.agent/plans/2026-11-16-app-update-notice.md): the daily check never
//! runs on the app's main thread and never holds up the caller, even when the server never answers.

// macOS only: Leon.app and its updater exist only there
#![cfg(target_os = "macos")]

mod app_update_kit;
mod release_kit;
mod update_kit;

use std::time::{Duration, Instant};

use app_update_kit::{until, Kit, T0};
use serde_json::json;
use update_kit::{Install, Mode, Release};

#[test]
fn a_server_that_never_answers_does_not_hold_up_the_app() {
    let k = Kit::with(Release::new, Install::app, json!({ "timeout_secs": 3 }));
    k.srv.set("latest.json", Mode::Stall);
    let u = k.updater();
    let start = Instant::now();
    u.daily(true, T0);
    let took = start.elapsed();
    assert!(took < Duration::from_millis(500), "daily() returned after {took:?}: it waited on the network");
    until("the request to be open", 30, || k.manifests() == 1);
    assert!(u.notice().is_none(), "nothing is drawn while a daily check runs");
}

#[test]
fn the_check_runs_on_the_updaters_own_thread() {
    let k = Kit::new();
    let u = k.updater();
    u.daily(true, T0);
    // the notice shows a moment before the wake that says so
    until("the notice and its wake", 30, || u.notice().is_some() && !k.woken_on.lock().unwrap().is_empty());
    let on = k.woken_on.lock().unwrap().clone();
    assert!(!on.is_empty(), "the check woke the app when it was done");
    let here = std::thread::current().name().unwrap_or("").to_string();
    assert!(on.iter().all(|t| t == "leon-update" && *t != here), "woken from {on:?}, the caller is {here:?}");
}
