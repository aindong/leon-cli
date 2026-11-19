//! GPUI phase 5 (ADR R8): no call to This Mac's server waits on the main thread. A fake server
//! answers the handshake and the model, then never answers the calls a window makes: a key, a
//! split (which asks the pane's working directory), a layout call, a copy, a cmd-click and Quit
//! and End All Sessions each return at once, where before each waited out the 5 s call timeout.
//! Each is timed alone, before the test scheduler runs the queue: in the app the queue runs on
//! GPUI's background threads, where an unanswered call holds only the queue. A cmd-click can't be
//! sent without running the queue in GPUI's test context; the scan below covers it with every
//! other call a view makes.
mod fake_support;
mod m10b_support;

use std::time::{Duration, Instant};

use fake_support::{open_fake, serve, Answer};
use gpui::{Keystroke, TestAppContext, VisualTestContext};
use leon_app::view::{QuitAndEndAll, Workspace};
use leon_core::search::Pos;
use m10b_support::*;

/// Each handler's limit: generous for a loaded machine, well under the 5 s a call waited.
const AT_ONCE: Duration = Duration::from_secs(1);

fn took(f: impl FnOnce()) -> Duration {
    let t = Instant::now();
    f();
    t.elapsed()
}

/// Run an app action by name and return, the queue not run.
fn action(ws: &gpui::Entity<Workspace>, cx: &mut VisualTestContext, name: &str) -> Duration {
    took(|| cx.update(|window, cx| ws.update(cx, |ws, cx| ws.run_action(name, None, "test", window, cx))))
}

#[gpui::test]
fn a_this_mac_server_that_never_answers_holds_nothing(cx: &mut TestAppContext) {
    let fake = serve(Answer::Model);
    let (ws, cx) = open_fake(cx, &fake);
    assert!(drawn(cx, "terminal-block:0"), "the pane is drawn");
    // first, while the queue is clear: the server's range (0:0 to 0:3, not the app's own guess of
    // the whole screen) replaces the one the app put on at once, so the reply came
    act(&ws, cx, "pane.select_all");
    let theirs = Some((Pos { line: 0, col: 0 }, Pos { line: 0, col: 3 }));
    let answered = |cx: &mut VisualTestContext| ws.read_with(cx, |ws, _| ws.selection_range("block:0")) == theirs;
    assert!(shows(cx, answered), "the selection's range came back: {:?}", fake.log.lock().unwrap());

    let mut typed = false;
    let key = took(|| typed = cx.update(|window, cx| window.dispatch_keystroke(Keystroke::parse("a").unwrap(), cx)));
    assert!(typed, "the key was handled");
    assert!(key < AT_ONCE, "a key took {key:?}");
    let split = action(&ws, cx, "pane.split.right");
    assert!(split < AT_ONCE, "a split (its cwd asked first) took {split:?}");
    let zoom = action(&ws, cx, "pane.zoom");
    assert!(zoom < AT_ONCE, "a layout call took {zoom:?}");
    let copy = action(&ws, cx, "pane.copy");
    assert!(copy < AT_ONCE, "a copy took {copy:?}");
    cx.update(|window, cx| window.dispatch_action(Box::new(QuitAndEndAll), cx));
    assert!(cx.has_pending_prompt(), "End All Sessions asks first");
    let quit = took(|| cx.simulate_prompt_answer("End All Sessions"));
    assert!(quit < AT_ONCE, "End All Sessions (its server.stop) took {quit:?}");
    let sent = fake.log.lock().unwrap().clone();
    assert!(!sent.iter().any(|m| m == "dev.leon.terminal.write"), "the key waits in the queue: {sent:?}");
    fake.hang_up(); // the queued calls fail at once now, not after their timeouts
}

/// No view file makes a call that waits for its reply (`Remote::call`, `act`, `lines`), but the
/// first session made before the window draws and the test-only helpers that say they wait.
#[test]
fn no_view_code_waits_for_a_server() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let allowed = [
        ("view_new.rs", "app.act(\"session.create\""),
        ("view_poll.rs", "local().call(\"client.keymap\""),
        ("view_poll.rs", "local().call(\"server.settings\""),
        ("view_scroll.rs", "self.app.lines("),
    ];
    for e in std::fs::read_dir(&src).unwrap().flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if !name.starts_with("view") {
            continue;
        }
        let text = std::fs::read_to_string(e.path()).unwrap();
        for line in text.lines().filter(|l| !l.trim_start().starts_with("//")) {
            // the workspace's own `self.call` queues (view_calls.rs); a remote's `call` and `act` wait
            let waits =
                ["app.call(", "().call(", "remote.call(", ".act(\"", "app.lines("].iter().any(|w| line.contains(w));
            let ok = allowed.iter().any(|(f, w)| *f == name && line.contains(w));
            assert!(!waits || ok, "{name} waits for a server on the window's thread: {}", line.trim());
        }
    }
}
