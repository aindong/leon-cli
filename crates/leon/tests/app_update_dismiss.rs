//! App update criterion 6 (.agent/plans/2026-11-16-app-update-notice.md): dismissing the notice
//! hides it for that version, across launches, and a newer version shows again.

// macOS only: Leon.app and its updater exist only there
#![cfg(target_os = "macos")]

mod app_update_kit;
mod release_kit;
mod update_kit;

use app_update_kit::{after, until, Kit, T0};
use leon_app::updates::DAY;
use update_kit::VERSION;

#[test]
fn dismiss_is_per_version() {
    let k = Kit::new();
    let u = k.updater();
    u.daily(true, T0);
    until("the notice", 30, || u.notice().is_some());
    u.dismiss();
    assert!(u.notice().is_none(), "dismissed");
    drop(u);

    // the next day, the same release: still dismissed
    let u = k.updater();
    u.daily(true, T0 + DAY);
    until("the second check", 30, || k.manifests() == 2);
    after(1);
    assert!(u.notice().is_none(), "{VERSION} stays dismissed: {:?}", u.notice().map(|n| n.text));
    drop(u);

    // a newer release the day after: shown
    k.rel.edit(".version = \"99.0.0\" | .tag = \"v99.0.0\"");
    let u = k.updater();
    u.daily(true, T0 + 2 * DAY);
    until("the newer notice", 30, || u.notice().is_some());
    assert!(u.notice().unwrap().text.contains("leon 99.0.0 is available"), "{}", u.notice().unwrap().text);
}
