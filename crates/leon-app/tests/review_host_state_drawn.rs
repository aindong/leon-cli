//! Review proof: a host whose attempt fails after the window settled is drawn in its new state.
mod m10b_support;

use std::os::unix::fs::PermissionsExt;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{px, size, TestAppContext, VisualTestContext};
use leon_app::view::{self, Workspace};
use m10b_support::*;

#[gpui::test]
fn a_host_that_fails_later_is_drawn_unreachable(cx: &mut TestAppContext) {
    let s = setup();
    let d = private_dir();
    let ssh = d.path().join("fake-ssh");
    std::fs::write(
        &ssh,
        "#!/bin/sh\nsleep 3\necho 'ssh: connect to host box port 22: Connection refused' >&2\nexit 255\n",
    )
    .unwrap();
    std::fs::set_permissions(&ssh, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::env::set_var("LEON_SSH", &ssh);
    std::env::set_var("LEON_TAILSCALE", "/nonexistent/tailscale");
    cx.update(view::init);
    let app = s.app.clone();
    let host = leon_hosts::Host { label: "box".into(), endpoints: vec!["ssh://dev@box".into()], icon: None };
    let (ws, cx) = cx.add_window_view(move |window, cx| {
        let remote = Rc::new(leon_app::remote::Remote::connect(app.socket()).expect("the app attaches"));
        Workspace::with_hosts(remote, None, vec![host], window, cx)
    });
    cx.simulate_resize(size(px(1200.), px(800.)));
    let failed = |cx: &mut VisualTestContext| {
        ws.read_with(cx, |ws, _| {
            ws.host_rows().iter().any(|r| r.label == "box" && format!("{:?}", r.state).contains("Unreachable"))
        })
    };
    let sidebar = |cx: &mut VisualTestContext| {
        ws.read_with(cx, |ws, _| ws.section_renders().get("sidebar").copied().unwrap_or(0))
    };
    // what opening brought in lands while the attempt is still out (it fails after 3 s)
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(1500) {
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!failed(cx), "the attempt is still out");
    let before = sidebar(cx);
    assert!(shows(cx, failed), "the attempt failed");
    cx.run_until_parked();
    let drew = sidebar(cx) - before;
    s.app.stop();
    assert!(
        drew >= 1,
        "the host row kept its old state: the sidebar drew {drew} times after the host became unreachable"
    );
}
