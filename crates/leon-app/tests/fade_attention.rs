//! The fade never hides what the user is waiting for: a pane that needs them (blocked, error, or a
//! done they haven't seen) is not faded, and a faded pane's status indicator keeps its colour.

mod m10b_support;

use gpui::{Entity, TestAppContext, VisualTestContext};
use leon_app::view::Workspace;
use m10b_support::*;

/// The default strength.
const S: f32 = leon_lua::settings::FADE as f32;
/// One step of GPUI's clock: a wait moves it on, drawing only the frames the app asks for.
const POLL: std::time::Duration = std::time::Duration::from_millis(8);

/// Two panes with the focus on the second; the first then reports `status` (OSC 7501) and keeps
/// running, so a new prompt doesn't clear it. Returns (the first pane, the second) once the
/// first's header shows its indicator.
fn reported(s: &Setup, ws: &Entity<Workspace>, cx: &mut VisualTestContext, status: &str) -> (String, String) {
    let first = ws.read_with(cx, |ws, _| ws.target_block()).expect("a pane");
    act(ws, cx, "pane.split.right");
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.target_block().is_some_and(|t| t != first))), "no split");
    let second = ws.read_with(cx, |ws, _| ws.target_block()).unwrap();
    let t = term(&s.app, &first);
    run_until(&t, &format!(r"printf '\033]7501;{status}\007'; echo sent; sleep 300"), "sent");
    assert!(shows(cx, |cx| drawn(cx, &format!("header-status-{first}"))), "{status}: no indicator in the header");
    (first, second)
}

/// The first pane's fade once the fades are at rest and it is `want` (else what it was).
fn rests_at(ws: &Entity<Workspace>, cx: &mut VisualTestContext, id: &str, want: f32) -> f32 {
    shows(cx, |cx| {
        tick(ws, cx, POLL);
        ws.read_with(cx, |ws, _| !ws.fading() && ws.pane_fade(id) == want)
    });
    ws.read_with(cx, |ws, _| ws.pane_fade(id))
}

fn check(cx: &mut TestAppContext, status: &str, want: f32, why: &str) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let (first, second) = reported(&s, &ws, cx, status);
    let got = rests_at(&ws, cx, &first, want);
    let focused = ws.read_with(cx, |ws, _| ws.pane_fade(&second));
    s.app.stop();
    assert_eq!(got, want, "{status}: {why}");
    assert_eq!(focused, 0., "{status}: the focused pane is at full strength");
}

#[gpui::test]
fn a_blocked_pane_is_not_faded(cx: &mut TestAppContext) {
    check(cx, "state=blocked:kind=question", 0., "it is waiting for the user");
}

#[gpui::test]
fn a_pane_with_an_error_is_not_faded(cx: &mut TestAppContext) {
    check(cx, "state=error", 0., "the user hasn't seen the error");
}

#[gpui::test]
fn a_pane_that_finished_unseen_is_not_faded(cx: &mut TestAppContext) {
    check(cx, "state=done", 0., "the user hasn't seen that it finished");
}

#[gpui::test]
fn a_working_pane_fades_but_its_indicator_keeps_its_colour(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let (first, second) = reported(&s, &ws, cx, "state=working");
    let got = rests_at(&ws, cx, &first, S);
    let (pane, focused, window) = ws.read_with(cx, |ws, _| (ws.pane_theme(&first), ws.pane_theme(&second), ws.theme()));
    let indicator = drawn(cx, &format!("header-status-{first}"));
    s.app.stop();
    assert_eq!(got, S, "working needs nobody: the pane fades");
    assert!(indicator, "its indicator is still in its header");
    assert_eq!(
        (pane.working, pane.done, pane.blocked, pane.error),
        (window.working, window.done, window.blocked, window.error),
        "the faded pane draws status in the window's own colours"
    );
    assert_eq!(pane, leon_app::fade::theme(&window, S), "the pane is drawn with the faded theme");
    for (name, faded, full) in [
        ("text", pane.text, window.text),
        ("bg", pane.bg, window.bg),
        ("selection", pane.selection, window.selection),
        ("found", pane.found, window.found),
        ("link", pane.link, window.link),
        ("cursor", pane.cursor, window.cursor),
    ] {
        assert_ne!(faded, full, "{name} fades with the pane");
    }
    assert_eq!(focused, window, "the focused pane is drawn with the window's theme");
}
