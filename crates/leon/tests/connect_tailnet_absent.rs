//! The Connect popover without a LocalAPI (.agent/plans/2026-11-22-sidebar-footer-connect.md,
//! criterion 5): no tailnet section, the rest as usual.

// macOS only: drives the GPUI app through support/app.rs (the `app` feature).
#![cfg(all(feature = "app", target_os = "macos"))]

#[path = "support/app.rs"]
mod app;
mod support;

use app::*;
use gpui::TestAppContext;
use support::localapi::Fake;

#[gpui::test]
fn no_localapi_no_tailnet_section(cx: &mut TestAppContext) {
    let h = hosted();
    let fake = Fake::new();
    std::env::set_var("LEON_TEST_TAILNET", fake.seam());
    fake.down();
    let (ws, cx) = open(cx, &h, vec![], None);
    act(&ws, cx, "client.connect", None);
    cx.run_until_parked();
    let labels: Vec<String> = ws
        .read_with(cx, |ws, cx| ws.connect_menu().unwrap().read(cx).items().iter().map(|i| i.label.clone()).collect());
    h.local.stop();
    assert!(!labels.contains(&"On your tailnet".to_string()), "{labels:?}");
    assert!(labels.contains(&"Add a host…".to_string()), "{labels:?}");
}
