//! GPUI phase 5 (ADR R8): no call to This Mac's server waits on the main thread. A fake server
//! answers the handshake and the model, then never answers the calls a window makes: a key, a
//! split (which asks the pane's working directory), a layout call, a copy, a cmd-click and Quit
//! and End All Sessions each return at once, where before each waited out the 5 s call timeout.
//! They wait on the calls' thread instead, one after another, which the window never sees.
mod fake_support;
mod m10b_support;

use std::time::{Duration, Instant};

use fake_support::{open_fake, serve, Answer};
use gpui::{Modifiers, MouseButton, TestAppContext};
use leon_app::view::QuitAndEndAll;
use m10b_support::*;

/// Each handler's limit: generous for a loaded machine, well under the 5 s a call waited.
const AT_ONCE: Duration = Duration::from_secs(1);

fn took(f: impl FnOnce()) -> Duration {
    let t = Instant::now();
    f();
    t.elapsed()
}

#[gpui::test]
fn a_this_mac_server_that_never_answers_holds_nothing(cx: &mut TestAppContext) {
    let fake = serve(Answer::Model);
    let opened = Instant::now();
    let (ws, cx) = open_fake(cx, &fake);
    // the grid fits its PTY from prepaint: a resize nobody answers
    assert!(opened.elapsed() < AT_ONCE * 3, "the window opened in {:?}", opened.elapsed());
    let area = at(cx, "terminal-block:0").expect("the pane is drawn");
    // first, while the queue is clear: every unanswered call after this holds it for its timeout
    act(&ws, cx, "pane.select_all");
    let ranged = shows(cx, |cx| ws.read_with(cx, |ws, _| ws.selection_range("block:0").is_some()));
    assert!(ranged, "the selection's range came back: {:?}", fake.log.lock().unwrap());

    let key = took(|| {
        cx.simulate_keystrokes("a");
        cx.run_until_parked();
    });
    assert!(key < AT_ONCE, "a key took {key:?}");

    let split = took(|| act(&ws, cx, "pane.split.right"));
    assert!(split < AT_ONCE, "a split (its cwd asked first) took {split:?}");

    let zoom = took(|| act(&ws, cx, "pane.zoom"));
    assert!(zoom < AT_ONCE, "a layout call took {zoom:?}");

    let copy = took(|| act(&ws, cx, "pane.copy"));
    assert!(copy < AT_ONCE, "a copy took {copy:?}");

    let click = took(|| {
        cx.simulate_mouse_down(
            gpui::point(area.origin.x + gpui::px(4.), area.origin.y + gpui::px(4.)),
            MouseButton::Left,
            Modifiers::command(),
        );
        cx.run_until_parked();
    });
    assert!(click < AT_ONCE, "a cmd-click (the link under it asked) took {click:?}");

    cx.dispatch_action(QuitAndEndAll);
    cx.run_until_parked();
    assert!(cx.has_pending_prompt(), "End All Sessions asks first");
    let quit = took(|| {
        cx.simulate_prompt_answer("End All Sessions");
        cx.run_until_parked();
    });
    // the stop waits on the calls' thread behind the calls above, each held for its timeout
    assert!(quit < AT_ONCE, "End All Sessions (its server.stop) took {quit:?}");
}
