//! App update criterion 10 (.agent/plans/2026-11-16-app-update-notice.md): the update notice is a
//! quiet card in the window, not a modal or a system notification. Each phase draws its own
//! controls, Dismiss hides it, Copy puts the brew command on the clipboard, a click leaves the keys
//! with the workspace, and the palette has the two commands.

mod m10b_support;

use gpui::{Entity, Modifiers, TestAppContext, VisualTestContext};
use leon_app::updates::{How, Phase};
use leon_app::view::Workspace;
use m10b_support::*;

const CONTROLS: [&str; 6] =
    ["update-install", "update-notes", "update-dismiss", "update-copy", "update-restart", "update-settings"];

fn show(ws: &Entity<Workspace>, cx: &mut VisualTestContext, phase: Phase) -> Vec<&'static str> {
    ws.read_with(cx, |ws, _| ws.updater().set_phase(phase));
    cx.run_until_parked();
    assert!(drawn(cx, "update-notice"), "the notice is drawn");
    CONTROLS.into_iter().filter(|c| drawn(cx, c)).collect()
}

fn available(how: How) -> Phase {
    Phase::Available { version: "9.9.9".into(), notes: "https://example.com/notes".into(), how }
}

#[gpui::test]
fn each_phase_draws_its_controls(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    assert!(!drawn(cx, "update-notice"), "nothing until there is something to say");
    assert_eq!(show(&ws, cx, available(How::Install)), ["update-install", "update-notes", "update-dismiss"]);
    let brew = show(&ws, cx, available(How::Brew("brew upgrade --cask aindong/leon/leon".into())));
    assert_eq!(brew, ["update-notes", "update-dismiss", "update-copy"]);
    assert_eq!(show(&ws, cx, available(How::MoveFirst)), ["update-notes", "update-dismiss"]);
    let working = Phase::Working { version: "9.9.9".into(), step: "downloading Leon.zip (12.3 MB)".into() };
    assert_eq!(show(&ws, cx, working), Vec::<&str>::new(), "no controls while it works");
    assert_eq!(show(&ws, cx, Phase::Done { version: "9.9.9".into() }), ["update-dismiss", "update-restart"]);
    let refused = Phase::Failed("can't replace /x/Leon.app: Operation not permitted (os error 1)".into());
    assert_eq!(show(&ws, cx, refused), ["update-dismiss", "update-settings"]);
    assert!(!cx.has_pending_prompt(), "never a modal");
    s.app.stop();
}

#[gpui::test]
fn dismiss_hides_it_and_the_keys_stay_with_the_workspace(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    show(&ws, cx, available(How::Install));
    let dismiss = at(cx, "update-dismiss").unwrap();
    cx.simulate_click(center(dismiss), Modifiers::none());
    assert!(!drawn(cx, "update-notice"), "dismissed");
    cx.simulate_keystrokes("cmd-shift-p");
    assert!(ws.read_with(cx, |ws, _| ws.palette_open()), "the workspace still has the keys after the click");
    s.app.stop();
}

#[gpui::test]
fn copy_puts_the_brew_command_on_the_clipboard(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    show(&ws, cx, available(How::Brew("brew upgrade --cask aindong/leon/leon".into())));
    let copy = at(cx, "update-copy").unwrap();
    cx.simulate_click(center(copy), Modifiers::none());
    let clip = cx.read_from_clipboard().and_then(|c| c.text());
    assert_eq!(clip.as_deref(), Some("brew upgrade --cask aindong/leon/leon"));
    s.app.stop();
}

#[gpui::test]
fn the_palette_has_check_and_install(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    for (query, title) in [("check for upd", "Check for Updates"), ("install upd", "Install Update")] {
        let rows = ws.read_with(cx, |ws, _| ws.palette_rows_for(query));
        assert!(rows.iter().any(|(t, _)| t == title), "{query}: {rows:?}");
    }
    s.app.stop();
}
