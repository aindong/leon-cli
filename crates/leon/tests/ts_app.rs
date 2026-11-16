//! Tailscale criterion 7 (.agent/plans/2026-11-16-tailscale-hosts.md): the app's Add Host… lists
//! the tailnet's machines with a search, adds the one chosen as a plain host, and the host's row
//! shows the tailnet identity. Without Tailscale it is today's prompt.

// macOS only: drives the GPUI app through support/app.rs (the `app` feature).
#![cfg(all(feature = "app", target_os = "macos"))]

#[path = "support/app.rs"]
mod app;
mod support;

use app::*;
use gpui::TestAppContext;
use support::tailscale::Fake;

fn rows(ws: &gpui::Entity<leon_app::view::Workspace>, cx: &mut gpui::VisualTestContext) -> String {
    let rows = ws.read_with(cx, |ws, _| ws.palette_rows());
    rows.into_iter().map(|(t, k)| format!("{t} | {}", k.unwrap_or_default())).collect::<Vec<_>>().join("\n")
}

#[gpui::test]
fn add_host_lists_the_tailnet(cx: &mut TestAppContext) {
    let h = hosted(); // LEON_TAILSCALE names a file that isn't there
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new().mode(0o700).create(h.r.local.state_dir()).unwrap();
    let (ws, cx) = open(cx, &h, vec![], None);

    act(&ws, cx, "client.host.add", None);
    assert!(rows(&ws, cx).to_lowercase().contains("label"), "no Tailscale: today's prompt: {}", rows(&ws, cx));
    cx.simulate_keystrokes("escape");

    let fake = Fake::new("running.json");
    std::env::set_var("PATH", fake.path());
    std::env::remove_var("LEON_TAILSCALE");
    act(&ws, cx, "client.host.add", None);
    assert!(shows(cx, |cx| rows(&ws, cx).contains("devbox.example-tailnet.ts.net")), "{}", rows(&ws, cx));
    let all = rows(&ws, cx);
    let line = |name: &str| all.lines().find(|l| l.contains(name)).unwrap().to_string();
    assert!(line("devbox.").contains("Tailscale SSH") && line("devbox.").contains("linux"), "{all}");
    assert!(line("laptop.").contains("offline"), "{all}");
    assert!(!all.contains("mymac"), "{all}");
    assert!(all.contains("me@example.com"), "who you are on the tailnet: {all}");
    assert!(all.contains("Another host"), "{all}");

    // the query searches them
    cx.simulate_input("build");
    let found = rows(&ws, cx);
    assert!(found.contains("buildbox.") && !found.contains("devbox.") && !found.contains("laptop."), "{found}");
    cx.simulate_keystrokes("enter");
    assert!(!ws.read_with(cx, |ws, _| ws.palette_open()), "added and closed: {}", rows(&ws, cx));
    assert_eq!(leon_hosts::hosts::load().unwrap(), vec![host("buildbox", "ssh://buildbox.example-tailnet.ts.net")]);
    let row = |cx: &mut gpui::VisualTestContext| {
        ws.read_with(cx, |ws, _| ws.host_rows()).into_iter().find(|r| r.label == "buildbox").unwrap()
    };
    assert!(
        shows(cx, |cx| row(cx).tailnet.as_deref() == Some("tailnet: me@example.com")),
        "the row shows the tailnet identity: {:?}",
        row(cx).tailnet
    );
    wait_connected(&ws, cx, "buildbox");
    assert_eq!(ws.read_with(cx, |ws, _| ws.host_rows())[0].tailnet, None, "This Mac has none");

    // a machine already added is refused as a duplicate label
    act(&ws, cx, "client.host.add", None);
    assert!(shows(cx, |cx| rows(&ws, cx).contains("buildbox.")));
    cx.simulate_input("buildbox");
    cx.simulate_keystrokes("enter");
    assert!(rows(&ws, cx).contains("already"), "{}", rows(&ws, cx));
    cx.simulate_keystrokes("escape");

    // with nothing matching, Enter goes on to today's prompt with the typed text
    act(&ws, cx, "client.host.add", None);
    cx.simulate_input("box9");
    assert!(!rows(&ws, cx).contains(".ts.net"), "{}", rows(&ws, cx));
    cx.simulate_keystrokes("enter");
    assert!(rows(&ws, cx).contains("ssh://"), "box9 was taken as the label: {}", rows(&ws, cx));
    cx.simulate_keystrokes("escape");
    assert_eq!(leon_hosts::hosts::load().unwrap().len(), 1);
    fake.only_read();
    h.local.stop();
}
