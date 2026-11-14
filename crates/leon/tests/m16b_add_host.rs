//! M16b criterion 6: "Add Host…" asks for a label and an endpoint, validates them as `leon hosts
//! add` does, saves through the hosts API and starts connecting.

// macOS only: drives the GPUI app through support/app.rs (the `app` feature).
#![cfg(all(feature = "app", target_os = "macos"))]

#[path = "support/app.rs"]
mod app;
mod support;

use app::*;
use gpui::TestAppContext;

fn rows(ws: &gpui::Entity<leon_app::view::Workspace>, cx: &mut gpui::VisualTestContext) -> String {
    ws.read_with(cx, |ws, _| ws.palette_rows()).into_iter().map(|(t, _)| t).collect::<Vec<_>>().join("\n")
}

#[gpui::test]
fn add_host_validates_and_saves(cx: &mut TestAppContext) {
    let h = hosted();
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new().mode(0o700).create(h.r.local.state_dir()).unwrap();
    let (ws, cx) = open(cx, &h, vec![], None);
    act(&ws, cx, "client.host.add", None);
    assert!(ws.read_with(cx, |ws, _| ws.palette_open()), "the prompt opens in the palette");
    assert!(rows(&ws, cx).to_lowercase().contains("label"), "it asks for a label: {}", rows(&ws, cx));

    cx.simulate_input("-x");
    cx.simulate_keystrokes("enter");
    assert!(rows(&ws, cx).contains("bad host label"), "{}", rows(&ws, cx));
    cx.simulate_keystrokes("backspace backspace");
    cx.simulate_input("box2");
    cx.simulate_keystrokes("enter");
    assert!(rows(&ws, cx).contains("ssh://"), "then for an endpoint: {}", rows(&ws, cx));

    cx.simulate_input("ssh://me@box2;x");
    cx.simulate_keystrokes("enter");
    assert!(rows(&ws, cx).contains("bad endpoint"), "{}", rows(&ws, cx));
    assert!(leon_hosts::hosts::load().unwrap().is_empty(), "nothing saved yet");
    cx.simulate_keystrokes("backspace backspace");
    cx.simulate_keystrokes("enter");
    assert!(!ws.read_with(cx, |ws, _| ws.palette_open()), "saved and closed");
    let saved = leon_hosts::hosts::load().unwrap();
    assert_eq!(saved, vec![host("box2", "ssh://me@box2")]);
    let labels: Vec<String> = ws.read_with(cx, |ws, _| ws.host_rows()).into_iter().map(|r| r.label).collect();
    assert_eq!(labels, ["This Mac", "box2"], "listed at once");
    wait_connected(&ws, cx, "box2");

    act(&ws, cx, "client.host.add", None);
    cx.simulate_input("box2");
    cx.simulate_keystrokes("enter");
    assert!(rows(&ws, cx).contains("already"), "a duplicate is refused: {}", rows(&ws, cx));
    cx.simulate_keystrokes("escape");
    assert_eq!(leon_hosts::hosts::load().unwrap().len(), 1);
    h.local.stop();
}
