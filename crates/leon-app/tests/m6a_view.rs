//! Criterion 8: the workspace view, headless: shortcuts, typing, paste, resize and the sidebar,
//! against a real embedded server and real shells.

use std::os::unix::fs::PermissionsExt;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{px, size, ClipboardItem, Entity, TestAppContext, VisualTestContext};
use leon_app::embedded::{self, Embedded};
use leon_app::view::{self, Workspace};
use leon_core::terminal::Terminal;

struct Setup {
    _dir: tempfile::TempDir,
    app: Rc<Embedded>,
}

fn setup() -> Setup {
    let dir = tempfile::Builder::new().permissions(std::fs::Permissions::from_mode(0o700)).tempdir().unwrap();
    let app = Rc::new(embedded::start_at(dir.path().join("server.sock")).unwrap());
    Setup { _dir: dir, app }
}

/// (session count, the workspace's session's window count, its active index).
fn shape(app: &Embedded, ws: &Entity<Workspace>, cx: &mut VisualTestContext) -> (usize, usize, usize) {
    let sid = ws.read_with(cx, |ws, _| ws.session_id()).expect("a session is selected");
    let st = app.ctx().state();
    let s = st.session(&sid).unwrap();
    (st.sessions.len(), s.windows.len(), s.active)
}

fn terminal(ws: &Entity<Workspace>, cx: &mut VisualTestContext) -> Arc<Terminal> {
    ws.read_with(cx, |ws, _| ws.terminal()).expect("the active window has a terminal")
}

fn wait_text(t: &Terminal, pred: impl Fn(&str) -> bool) -> String {
    let end = Instant::now() + Duration::from_secs(10);
    loop {
        let text = t.text(true);
        if pred(&text) || Instant::now() > end {
            return text;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn has_line(text: &str, line: &str) -> bool {
    text.lines().any(|l| l.trim() == line)
}

fn open<'a>(cx: &'a mut TestAppContext, app: &Rc<Embedded>) -> (Entity<Workspace>, &'a mut VisualTestContext) {
    cx.update(view::init);
    let app = app.clone();
    let (ws, vcx) = cx.add_window_view(move |window, cx| Workspace::new(app, window, cx));
    vcx.run_until_parked();
    (ws, vcx)
}

#[gpui::test]
fn tab_and_session_shortcuts(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    assert_eq!(shape(&s.app, &ws, cx), (1, 1, 0), "the app starts a session");
    cx.simulate_keystrokes("cmd-t");
    assert_eq!(shape(&s.app, &ws, cx), (1, 2, 1), "cmd-t adds a tab and selects it");
    cx.simulate_keystrokes("cmd-1");
    assert_eq!(shape(&s.app, &ws, cx).2, 0);
    cx.simulate_keystrokes("cmd-shift-]");
    assert_eq!(shape(&s.app, &ws, cx).2, 1);
    cx.simulate_keystrokes("cmd-shift-[");
    assert_eq!(shape(&s.app, &ws, cx).2, 0);
    cx.simulate_keystrokes("cmd-w");
    assert_eq!(shape(&s.app, &ws, cx), (1, 1, 0), "cmd-w closes the tab");
    let first = ws.read_with(cx, |ws, _| ws.session_id());
    cx.simulate_keystrokes("cmd-n");
    let (sessions, _, _) = shape(&s.app, &ws, cx);
    assert_eq!(sessions, 2);
    assert_ne!(ws.read_with(cx, |ws, _| ws.session_id()), first, "cmd-n selects the new session");
    s.app.stop();
}

#[gpui::test]
fn typing_and_paste_reach_the_shell(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let t = terminal(&ws, cx);
    cx.simulate_input("echo hi");
    cx.simulate_keystrokes("enter");
    let text = wait_text(&t, |x| has_line(x, "hi"));
    assert!(has_line(&text, "hi"), "{text}");
    cx.write_to_clipboard(ClipboardItem::new_string("echo pa\necho pb\n".into()));
    cx.simulate_keystrokes("cmd-v");
    // A shell with bracketed paste (zsh) holds pasted lines until Enter; plain sh ran them already.
    cx.simulate_keystrokes("enter");
    let text = wait_text(&t, |x| has_line(x, "pa") && has_line(x, "pb"));
    assert!(has_line(&text, "pa") && has_line(&text, "pb"), "{text}");
    cx.simulate_keystrokes("ctrl-c"); // reaches the shell, doesn't break anything
    s.app.stop();
}

#[gpui::test]
fn window_resize_resizes_the_pty(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let t = terminal(&ws, cx);
    cx.simulate_resize(size(px(700.), px(400.)));
    cx.run_until_parked();
    let small = t.size();
    cx.simulate_resize(size(px(1400.), px(900.)));
    cx.run_until_parked();
    let big = t.size();
    assert!(big.0 > small.0 && big.1 > small.1, "{small:?} -> {big:?}");
    t.write(b"stty size\r").unwrap();
    let want = format!("{} {}", big.0, big.1);
    let text = wait_text(&t, |x| has_line(x, &want));
    assert!(has_line(&text, &want), "wanted {want}: {text}");
    s.app.stop();
}

#[gpui::test]
fn cmd_b_toggles_the_sidebar(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    assert!(ws.read_with(cx, |ws, _| ws.sidebar_visible()));
    let t = terminal(&ws, cx);
    let with_sidebar = t.size().1;
    cx.simulate_keystrokes("cmd-b");
    cx.run_until_parked();
    assert!(!ws.read_with(cx, |ws, _| ws.sidebar_visible()));
    assert!(t.size().1 > with_sidebar, "the terminal takes the sidebar's width");
    cx.simulate_keystrokes("cmd-b");
    assert!(ws.read_with(cx, |ws, _| ws.sidebar_visible()));
    s.app.stop();
}

#[gpui::test]
fn closing_the_last_tab_leaves_an_empty_app_that_cmd_n_recovers(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    cx.simulate_keystrokes("cmd-w");
    assert!(s.app.ctx().state().sessions.is_empty(), "the last tab took its session with it");
    assert_eq!(ws.read_with(cx, |ws, _| ws.session_id()), None);
    assert!(ws.read_with(cx, |ws, _| ws.terminal()).is_none());
    cx.simulate_keystrokes("a enter"); // no terminal: keys go nowhere, nothing panics
    cx.simulate_keystrokes("cmd-n");
    assert_eq!(s.app.ctx().state().sessions.len(), 1);
    assert!(ws.read_with(cx, |ws, _| ws.terminal()).is_some());
    s.app.stop();
}
