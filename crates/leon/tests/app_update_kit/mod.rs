//! Shared by the app_update_* tests: Leon.app's updater (`leon_app::updates`) run in this process
//! against part A's loopback release server, updating a leon installed in a temp dir.
//!
//! The updater reads `LEON_RELEASE_URL` and the debug-only `LEON_TEST_UPDATE` as `leon
//! self-update` does, so they are set here, for the whole process; `Kit` holds a lock so the tests
//! of one binary take turns. The release and the install are made under it too: `Install::at`
//! looks up curl on PATH, which a test may have changed.

#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use leon_app::updates::{Notice, Updater};
use serde_json::{json, Value};

use crate::update_kit::{Install, Release, Server, INFO_PLIST, OLD};

static TURN: Mutex<()> = Mutex::new(());

/// Any time after the epoch will do: the tests move their own clock.
pub const T0: u64 = 1_800_000_000;

pub struct Kit {
    pub rel: Release,
    pub srv: Server,
    pub leon: Install,
    /// The updater's file (`updates.json`).
    pub store: PathBuf,
    /// Every notice text, in order, as each change woke the app.
    pub seen: Arc<Mutex<Vec<String>>>,
    /// The name of the thread each wake came from.
    pub woken_on: Arc<Mutex<Vec<String>>>,
    _turn: MutexGuard<'static, ()>,
}

impl Kit {
    /// A newer release, served, and a Leon.app in a temp dir that is version `OLD`.
    pub fn new() -> Kit {
        Kit::with(Release::new, Install::app, json!({}))
    }

    /// `rel` served to `leon`, with `seam` over the usual one (trust `rel`'s key, be `OLD`).
    pub fn with(rel: impl FnOnce() -> Release, leon: impl FnOnce() -> Install, seam: Value) -> Kit {
        let turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
        let (rel, leon) = (rel(), leon());
        let srv = rel.serve();
        let mut full = json!({ "keys": rel.key.keys_file, "version": OLD });
        for (k, v) in seam.as_object().unwrap() {
            full[k] = v.clone();
        }
        std::env::set_var("LEON_RELEASE_URL", srv.url());
        std::env::set_var("LEON_TEST_UPDATE", full.to_string());
        std::env::remove_var("HOMEBREW_PREFIX");
        let store = leon.home.join("state/updates.json");
        Kit { rel, srv, leon, store, seen: Arc::default(), woken_on: Arc::default(), _turn: turn }
    }

    /// An updater for this kit's install and store, as a fresh launch of the app makes one.
    pub fn updater(&self) -> Arc<Updater> {
        let exe = self.leon.exe.clone();
        let (seen, woken_on) = (self.seen.clone(), self.woken_on.clone());
        let slot: Arc<Mutex<Option<std::sync::Weak<Updater>>>> = Arc::default();
        let back = slot.clone();
        let wake = Arc::new(move || {
            woken_on.lock().unwrap().push(std::thread::current().name().unwrap_or("").to_string());
            let me = back.lock().unwrap().as_ref().and_then(|w| w.upgrade());
            if let Some(text) = me.and_then(|u| u.notice()).map(|n| n.text) {
                let mut seen = seen.lock().unwrap();
                if seen.last() != Some(&text) {
                    seen.push(text);
                }
            }
        });
        let target = Arc::new(move || leon_update::Target::at(exe.clone()));
        let u = Arc::new(Updater::new(Some(self.store.clone()), target, wake));
        *slot.lock().unwrap() = Some(Arc::downgrade(&u));
        u
    }

    /// How many times the manifest was asked for.
    pub fn manifests(&self) -> usize {
        self.srv.asked().iter().filter(|n| *n == "latest.json").count()
    }
}

/// A Leon.app at `rel` (under the temp dir), with leon's bundle id.
pub fn app_at(rel: &str) -> Install {
    let mut i = Install::at(&format!("{rel}/Contents/MacOS/leon"));
    i.root = i.home.join(rel);
    std::fs::write(i.root.join("Contents/Info.plist"), INFO_PLIST).unwrap();
    i
}

/// Wait up to `secs` for `pred`, panicking with `what` if it never holds.
pub fn until(what: &str, secs: u64, mut pred: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(secs);
    while !pred() {
        assert!(Instant::now() < end, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// A negative needs a window of time: let `secs` go by, then look.
pub fn after(secs: u64) {
    std::thread::sleep(Duration::from_secs(secs));
}

/// The notice the updater shows now.
pub fn notice(u: &Updater) -> Option<Notice> {
    u.notice()
}
