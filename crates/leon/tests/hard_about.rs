//! Audit A14 (docs/audit-0.1.0-app.md): the app's menu has "About leon", which shows the version
//! as `leon --version` prints it (with the commit), passed in by `leon app`.

// macOS only: drives the GPUI app through support/app.rs (the `app` feature).
#![cfg(all(feature = "app", target_os = "macos"))]

#[path = "support/app.rs"]
mod app;
mod support;

use app::*;
use gpui::{MenuItem, TestAppContext};

#[gpui::test]
fn about_shows_what_leon_version_prints(cx: &mut TestAppContext) {
    let h = hosted();
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_leon")).arg("--version").output().unwrap();
    let version = String::from_utf8(out.stdout).unwrap().trim().to_string();
    assert!(version.starts_with("leon "), "{version}");
    let items = &leon_app::view::menus()[0].items;
    let about = items.iter().find_map(|i| match i {
        MenuItem::Action { name, action, .. } if name.as_ref() == "About leon" => Some(action.boxed_clone()),
        _ => None,
    });
    let about = about.expect("the leon menu has About leon");
    leon_app::view::set_version(env!("LEON_VERSION")); // what `leon app` passes in
    let (_ws, cx) = open(cx, &h, vec![], None);
    cx.update(|window, cx| window.dispatch_action(about, cx));
    cx.run_until_parked();
    let (message, _) = cx.pending_prompt().expect("About shows a prompt");
    assert_eq!(message, version);
    h.local.stop();
}
