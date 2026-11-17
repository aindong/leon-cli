//! Tailscale review (skeptic): the tailnet's status is read off the UI thread; once it comes, the
//! window must draw it, even when nothing else changed.

// macOS only: drives the GPUI app through support/app.rs (the `app` feature).
#![cfg(all(feature = "app", target_os = "macos"))]

#[path = "support/app.rs"]
mod app;
mod support;

use std::os::unix::fs::PermissionsExt;

use app::*;
use gpui::TestAppContext;
use leon_app::hosts::HostState;

#[gpui::test]
fn the_tailnet_line_is_drawn_once_the_status_comes(cx: &mut TestAppContext) {
    let h = hosted();
    // a client that answers after the window's first frames (under the 3 s deadline)
    let ts = tempfile::tempdir().unwrap();
    let bin = ts.path().join("tailscale");
    std::fs::write(&bin, "#!/bin/sh\nsleep 1.5\ncat \"$(dirname \"$0\")/status\"\n").unwrap();
    std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::copy(support::tailscale::fixture("running.json"), ts.path().join("status")).unwrap();
    std::env::set_var("PATH", format!("{}:{}", ts.path().display(), std::env::var("PATH").unwrap_or_default()));
    std::env::remove_var("LEON_TAILSCALE");
    // ssh waits, so no host state change draws a frame after the status comes
    let hold = h.r.fake.path().join("hold");
    std::fs::write(&hold, "").unwrap();

    let (ws, cx) = open(cx, &h, vec![host("devbox", "ssh://devbox.example-tailnet.ts.net")], None);
    let known = shows(cx, |cx| ws.read_with(cx, |ws, _| ws.host_rows()[1].tailnet.is_some()));
    let still = state(&ws, cx, "devbox");
    let painted = shows(cx, |cx| drawn(cx, "host-tailnet-1"));
    std::fs::remove_file(&hold).unwrap();
    assert!(known, "the status came in");
    assert_eq!(still, Some(HostState::Connecting));
    assert!(painted, "the status came in but the window never drew the tailnet line");
    h.local.stop();
}
