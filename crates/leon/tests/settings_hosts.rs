//! Settings › Remote Hosts: each host with its endpoint, its state and why (unreachable, needs
//! login) and its tailnet login; Rename (a taken label refused inline), Remove (asks first), and
//! Add Host… opening the main window's prompt. Hosts go through the fake ssh; Tailscale is a fake.

// macOS only: drives the GPUI app through support/app.rs (the `app` feature).
#![cfg(all(feature = "app", target_os = "macos"))]

#[path = "support/app.rs"]
mod app;
mod support;

use app::*;
use gpui::{Modifiers, TestAppContext, VisualTestContext};
use leon_app::settings_window::SettingsWindow;
use support::tailscale::Fake;

fn click(cx: &mut VisualTestContext, sel: &'static str) {
    cx.run_until_parked();
    let b = cx.debug_bounds(sel).unwrap_or_else(|| panic!("nothing drawn as {sel}"));
    cx.simulate_click(b.center(), Modifiers::none());
    cx.run_until_parked();
}

fn labels() -> Vec<String> {
    leon_hosts::hosts::load().unwrap().into_iter().map(|h| h.label).collect()
}

#[gpui::test]
fn remote_hosts_lists_renames_removes_and_adds(cx: &mut TestAppContext) {
    let h = hosted();
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new().mode(0o700).create(h.r.local.state_dir()).unwrap();
    let fake = Fake::new("running.json");
    std::env::set_var("PATH", fake.path());
    std::env::remove_var("LEON_TAILSCALE");
    let hosts = vec![
        host("unreachable-box", "ssh://unreachable-box"),
        host("login-box", "ssh://login-box"),
        host("buildbox", "ssh://buildbox.example-tailnet.ts.net"),
    ];
    leon_hosts::hosts::save(&hosts).unwrap();
    let (ws, wcx) = open(cx, &h, hosts, None);
    let remote = std::rc::Rc::new(leon_app::remote::Remote::connect(h.local.socket()).unwrap());
    let config = h.dir.path().join("init.lua");
    let sw = wcx.update(|_, cx| SettingsWindow::open(config, remote, cx)).unwrap();
    let mut cx = VisualTestContext::from_window(sw.into(), wcx);
    cx.simulate_resize(gpui::size(gpui::px(900.), gpui::px(2400.)));
    let cx = &mut cx;
    let line = |cx: &mut VisualTestContext, label: &str| {
        sw.read_with(cx, |sw, _| sw.host_lines()).unwrap().into_iter().find(|l| l.label == label)
    };
    let why = |cx: &mut VisualTestContext, label: &str| line(cx, label).and_then(|l| l.note).unwrap_or_default();
    assert!(
        shows(cx, |cx| why(cx, "unreachable-box").contains("Connection refused")),
        "{:?}",
        line(cx, "unreachable-box")
    );
    assert!(why(cx, "unreachable-box").starts_with("unreachable"));
    assert!(shows(cx, |cx| why(cx, "login-box").starts_with("needs login")), "{:?}", line(cx, "login-box"));
    assert_eq!(line(cx, "buildbox").unwrap().endpoint, "ssh://buildbox.example-tailnet.ts.net");
    assert!(shows(cx, |cx| line(cx, "buildbox").unwrap().tailnet.as_deref() == Some("tailnet: me@example.com")));
    assert!(shows(cx, |cx| line(cx, "buildbox").unwrap().connected), "{:?}", line(cx, "buildbox"));
    click(cx, "section-hosts");
    let shown = sw.read_with(cx, |sw, _| sw.shown()).unwrap();
    assert_eq!(shown, ["hosts", "host:unreachable-box", "host:login-box", "host:buildbox"]);

    // Rename: a taken label is refused where it was typed, and nothing changes
    click(cx, "host-rename-2");
    for _ in 0.."buildbox".len() {
        cx.simulate_keystrokes("backspace");
    }
    cx.simulate_input("login-box");
    cx.simulate_keystrokes("enter");
    let err = sw.read_with(cx, |sw, _| sw.rename_error()).unwrap();
    assert!(err.as_deref().is_some_and(|e| e.contains("already")), "{err:?}");
    cx.run_until_parked();
    assert!(cx.debug_bounds("host-rename-error").is_some(), "the refusal is drawn under the field");
    assert_eq!(labels(), ["unreachable-box", "login-box", "buildbox"]);
    for _ in 0.."login-box".len() {
        cx.simulate_keystrokes("backspace");
    }
    cx.simulate_input("ci");
    cx.simulate_keystrokes("enter");
    assert_eq!(labels(), ["unreachable-box", "login-box", "ci"]);
    let rows: Vec<String> = ws.read_with(cx, |ws, _| ws.host_rows()).into_iter().map(|r| r.label).collect();
    assert!(rows.contains(&"ci".to_string()) && !rows.contains(&"buildbox".to_string()), "{rows:?}");
    assert!(shows(cx, |cx| line(cx, "ci").is_some()), "Settings follows the main window");

    // Remove asks first; Cancel keeps it
    click(cx, "host-remove-0");
    let detail = "leon stops showing it and forgets its address. Its server and the programs in its \
                  sessions keep running on that machine.";
    assert_eq!(cx.pending_prompt(), Some(("Remove unreachable-box?".to_string(), detail.to_string())));
    cx.simulate_prompt_answer("Cancel");
    cx.run_until_parked();
    assert_eq!(labels().len(), 3, "Cancel keeps it");
    click(cx, "host-remove-0");
    cx.simulate_prompt_answer("Remove");
    cx.run_until_parked();
    assert_eq!(labels(), ["login-box", "ci"]);
    let rows: Vec<String> = ws.read_with(cx, |ws, _| ws.host_rows()).into_iter().map(|r| r.label).collect();
    assert_eq!(rows, ["This Mac", "login-box", "ci"]);
    assert!(shows(cx, |cx| line(cx, "unreachable-box").is_none()));

    // search finds a host by its label
    cx.simulate_input("login");
    let found = sw.read_with(cx, |sw, _| sw.shown()).unwrap();
    assert_eq!(found, ["host:login-box"]);
    cx.simulate_keystrokes("escape");
    click(cx, "section-hosts");

    // Add Host… opens the main window's prompt (Tailscale's machines first)
    click(cx, "hosts-add");
    let open = ws.read_with(cx, |ws, _| ws.palette_open());
    fake.only_read();
    h.local.stop();
    assert!(open, "the Add Host prompt opened in the main window");
}
