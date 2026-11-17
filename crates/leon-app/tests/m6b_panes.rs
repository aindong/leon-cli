//! M6b criteria 2–6: the workspace renders and drives split layouts, headless, against a real
//! embedded server and real shells.

mod m9_support;

use std::os::unix::fs::PermissionsExt;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{
    point, px, size, Bounds, ClipboardItem, Entity, Modifiers, MouseButton, Pixels, TestAppContext, VisualTestContext,
};
use leon_app::embedded::{self, Embedded};
use leon_app::view::{self, Workspace};
use leon_core::layout::{Direction, Node};
use leon_core::terminal::Terminal;
use leon_core::Window;

struct Setup {
    _dir: tempfile::TempDir,
    app: Rc<Embedded>,
}

fn setup() -> Setup {
    let dir = tempfile::Builder::new().permissions(std::fs::Permissions::from_mode(0o700)).tempdir().unwrap();
    // a plain shell: the tests type into panes just made
    let app = Rc::new(embedded::start_with(dir.path().join("server.sock"), None, m9_support::plain_env()).unwrap());
    Setup { _dir: dir, app }
}

fn open<'a>(cx: &'a mut TestAppContext, app: &Rc<Embedded>) -> (Entity<Workspace>, &'a mut VisualTestContext) {
    cx.update(view::init);
    let app = app.clone();
    let (ws, vcx) = cx.add_window_view(move |window, cx| {
        Workspace::new(Rc::new(leon_app::remote::Remote::connect(app.socket()).expect("the app attaches")), window, cx)
    });
    vcx.simulate_resize(size(px(1400.), px(900.)));
    vcx.run_until_parked();
    (ws, vcx)
}

/// The selected session's active window, and how many tabs the session has.
fn window(app: &Embedded, ws: &Entity<Workspace>, cx: &mut VisualTestContext) -> (Window, usize) {
    let sid = ws.read_with(cx, |ws, _| ws.session_id()).expect("a session is selected");
    let st = app.ctx().state();
    let s = st.session(&sid).unwrap();
    (s.windows[s.active].clone(), s.windows.len())
}

fn leaves(w: &Window) -> Vec<String> {
    w.layout.as_ref().map(|t| t.leaves().into_iter().map(String::from).collect()).unwrap_or_default()
}

fn ratio(w: &Window) -> f64 {
    match w.layout.as_ref().unwrap() {
        Node::Split { ratio, .. } => *ratio,
        Node::Block { .. } => panic!("not split"),
    }
}

fn term(app: &Embedded, block_id: &str) -> Arc<Terminal> {
    app.ctx().state().block(block_id).unwrap().1.terminal.clone()
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

/// Until `t` printed something and then stayed quiet for 300 ms.
fn settle(t: &Terminal) {
    wait_text(t, |x| !x.trim().is_empty());
    let end = Instant::now() + Duration::from_secs(10);
    let (mut last, mut since) = (t.generation(), Instant::now());
    while Instant::now() < end && since.elapsed() < Duration::from_millis(300) {
        std::thread::sleep(Duration::from_millis(20));
        if t.generation() != last {
            (last, since) = (t.generation(), Instant::now());
        }
    }
}

/// Whether the signature moves off `before` within 5 s: changes made outside the app reach it as
/// events and pushed screens, a moment later.
fn changes(cx: &mut VisualTestContext, sig: &dyn Fn(&mut VisualTestContext) -> u64, before: u64) -> bool {
    let end = Instant::now() + Duration::from_secs(5);
    while sig(cx) == before && Instant::now() < end {
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(10));
    }
    sig(cx) != before
}

fn center(b: Bounds<Pixels>) -> gpui::Point<Pixels> {
    point(b.origin.x + b.size.width / 2., b.origin.y + b.size.height / 2.)
}

#[gpui::test]
fn split_focus_zoom_and_close_shortcuts(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let a = leaves(&window(&s.app, &ws, cx).0)[0].clone();

    cx.simulate_keystrokes("cmd-d");
    let (w, _) = window(&s.app, &ws, cx);
    let l = leaves(&w);
    assert_eq!(l.len(), 2);
    assert!(matches!(w.layout, Some(Node::Split { direction: Direction::Horizontal, .. })), "cmd-d splits right");
    assert_eq!(w.focus.as_deref(), Some(l[1].as_str()), "the new pane is focused");
    let b = l[1].clone();

    cx.simulate_keystrokes("cmd-shift-d");
    let (w, _) = window(&s.app, &ws, cx);
    let l = leaves(&w);
    assert_eq!(l.len(), 3);
    let c = l[2].clone();
    assert_eq!(w.focus.as_deref(), Some(c.as_str()));
    match w.layout.as_ref().unwrap() {
        Node::Split { after, .. } => {
            assert!(matches!(**after, Node::Split { direction: Direction::Vertical, .. }), "cmd-shift-d splits below")
        }
        _ => panic!("not split"),
    }

    cx.simulate_keystrokes("cmd-alt-up");
    assert_eq!(window(&s.app, &ws, cx).0.focus.as_deref(), Some(b.as_str()));
    cx.simulate_keystrokes("cmd-alt-left");
    assert_eq!(window(&s.app, &ws, cx).0.focus.as_deref(), Some(a.as_str()));
    cx.simulate_keystrokes("cmd-alt-right");
    assert_ne!(window(&s.app, &ws, cx).0.focus.as_deref(), Some(a.as_str()));
    cx.simulate_keystrokes("cmd-alt-down");
    assert_eq!(window(&s.app, &ws, cx).0.focus.as_deref(), Some(c.as_str()));

    cx.simulate_keystrokes("cmd-shift-enter");
    assert_eq!(window(&s.app, &ws, cx).0.zoomed.as_deref(), Some(c.as_str()));
    cx.simulate_keystrokes("cmd-shift-enter");
    assert_eq!(window(&s.app, &ws, cx).0.zoomed, None);

    cx.simulate_keystrokes("cmd-t"); // a second tab, so closing the first's panes can close it
    cx.simulate_keystrokes("cmd-1");
    cx.simulate_keystrokes("cmd-w");
    let (w, tabs) = window(&s.app, &ws, cx);
    assert_eq!((leaves(&w).len(), tabs), (2, 2), "cmd-w closes the focused pane only");
    assert!(!leaves(&w).contains(&c));
    cx.simulate_keystrokes("cmd-w");
    cx.simulate_keystrokes("cmd-w");
    assert_eq!(window(&s.app, &ws, cx).1, 1, "the last pane's cmd-w closes the tab");
    s.app.stop();
}

#[gpui::test]
fn input_goes_to_the_focused_pane(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let old = leaves(&window(&s.app, &ws, cx).0)[0].clone();
    cx.simulate_keystrokes("cmd-d");
    let new = window(&s.app, &ws, cx).0.focus.unwrap();
    let (t_new, t_old) = (term(&s.app, &new), term(&s.app, &old));
    settle(&t_new);
    cx.simulate_input("echo hi");
    cx.simulate_keystrokes("enter");
    let text = wait_text(&t_new, |x| has_line(x, "hi"));
    assert!(has_line(&text, "hi"), "{text}");
    assert!(!has_line(&t_old.text(true), "hi"), "the other pane got nothing");
    cx.write_to_clipboard(ClipboardItem::new_string("echo pz\n".into()));
    cx.simulate_keystrokes("cmd-v");
    cx.simulate_keystrokes("enter");
    let text = wait_text(&t_new, |x| has_line(x, "pz"));
    assert!(has_line(&text, "pz"), "{text}");

    // Focus, not age, picks the pane: back to the old one, then a click on the new one.
    cx.simulate_keystrokes("cmd-alt-left");
    settle(&t_old);
    cx.simulate_input("echo back");
    cx.simulate_keystrokes("enter");
    let text = wait_text(&t_old, |x| has_line(x, "back"));
    assert!(has_line(&text, "back"), "{text}");
    assert!(!has_line(&t_new.text(true), "back"));
    let pane = cx.debug_bounds(Box::leak(format!("pane-{new}").into_boxed_str())).expect("pane bounds");
    cx.simulate_click(center(pane), Modifiers::none());
    cx.simulate_input("echo again");
    cx.simulate_keystrokes("enter");
    let text = wait_text(&t_new, |x| has_line(x, "again"));
    assert!(has_line(&text, "again"), "{text}");
    assert!(!has_line(&t_old.text(true), "again"));
    s.app.stop();
}

#[gpui::test]
fn zoom_never_hides_where_input_goes(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    cx.simulate_keystrokes("cmd-d");
    cx.simulate_keystrokes("cmd-alt-left");
    cx.simulate_keystrokes("cmd-shift-enter"); // zoom the left pane
    cx.simulate_keystrokes("cmd-alt-right"); // unzooms, then moves focus
    let w = window(&s.app, &ws, cx).0;
    assert_eq!(w.zoomed, None, "focus by direction unzooms");
    let right = w.focus.unwrap();
    let t = term(&s.app, &right);
    settle(&t);
    cx.simulate_input("echo z");
    cx.simulate_keystrokes("enter");
    assert!(has_line(&wait_text(&t, |x| has_line(x, "z")), "z"), "typed into the visible, focused pane");

    cx.simulate_keystrokes("cmd-shift-enter");
    cx.simulate_keystrokes("cmd-d"); // unzooms, then splits
    cx.run_until_parked();
    let w = window(&s.app, &ws, cx).0;
    assert_eq!((w.zoomed.clone(), leaves(&w).len()), (None, 3));
    let new = term(&s.app, &w.focus.clone().unwrap());
    settle(&new);
    let (rows, cols) = new.size();
    let beside = term(&s.app, &leaves(&w)[0]).size();
    assert_eq!(rows, beside.0, "the new pane was laid out (not left at the spawn size): {rows}x{cols}");
    assert!(cols < 120, "the new pane is laid out beside the others: {cols}");
    new.write(b"stty size\r").unwrap();
    let want = format!("{rows} {cols}");
    assert!(has_line(&wait_text(&new, |x| has_line(x, &want)), &want), "stty size is the pane's own");

    // A block zoomed from the CLI, not the focused one: input follows what's on screen.
    let (sid, other) = (ws.read_with(cx, |ws, _| ws.session_id()).unwrap(), leaves(&w)[0].clone());
    s.app
        .call("session.zoom_block", serde_json::json!({ "session_id": sid, "block_id": other, "zoom": true }))
        .unwrap();
    // the app hears of it by an event and a refetch, a moment later: type once it shows it
    let zoomed = |cx: &mut VisualTestContext| ws.read_with(cx, |ws, _| ws.target_block()) == Some(other.clone());
    assert!(m9_support::shows(cx, zoomed), "the app shows the zoom from the CLI");
    let shown = term(&s.app, &other);
    settle(&shown);
    cx.simulate_input("echo seen");
    cx.simulate_keystrokes("enter");
    assert!(has_line(&wait_text(&shown, |x| has_line(x, "seen")), "seen"), "typed into the zoomed block");
    s.app.stop();
}

#[gpui::test]
fn each_pane_has_its_own_pty_size(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let a = leaves(&window(&s.app, &ws, cx).0)[0].clone();
    let full = term(&s.app, &a).size();
    cx.simulate_keystrokes("cmd-d");
    cx.run_until_parked();
    let b = window(&s.app, &ws, cx).0.focus.unwrap();
    let (sa, sb) = (term(&s.app, &a).size(), term(&s.app, &b).size());
    assert_eq!((sa.0, sb.0), (full.0, full.0), "rows stay: {full:?} {sa:?} {sb:?}");
    let most = (full.1 as f32 * 0.6) as u16;
    assert!(sa.1 < most && sb.1 < most && sa.1 > 1 && sb.1 > 1, "columns split: {full:?} {sa:?} {sb:?}");
    let t = term(&s.app, &b);
    settle(&t);
    t.write(b"stty size\r").unwrap();
    let want = format!("{} {}", sb.0, sb.1);
    let text = wait_text(&t, |x| has_line(x, &want));
    assert!(has_line(&text, &want), "wanted {want}: {text}");
    cx.simulate_keystrokes("cmd-shift-enter");
    cx.run_until_parked();
    assert_eq!(term(&s.app, &b).size(), full, "the zoomed pane gets the whole area");
    s.app.stop();
}

#[gpui::test]
fn dragging_an_inner_divider_uses_its_own_split(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    cx.simulate_keystrokes("cmd-d");
    cx.simulate_keystrokes("cmd-shift-d"); // A | (B / C), focus on C
    cx.run_until_parked();
    let w = window(&s.app, &ws, cx).0;
    let focus = w.focus.clone();
    let (outer, inner) = match w.layout.as_ref().unwrap() {
        Node::Split { ratio, after, .. } => match &**after {
            Node::Split { ratio: r, .. } => (*ratio, *r),
            _ => panic!("no inner split"),
        },
        _ => panic!("not split"),
    };
    let (b, c) = (leaves(&w)[1].clone(), leaves(&w)[2].clone());
    let pb = cx.debug_bounds(Box::leak(format!("pane-{b}").into_boxed_str())).unwrap();
    let pc = cx.debug_bounds(Box::leak(format!("pane-{c}").into_boxed_str())).unwrap();
    let split_h = f32::from(pc.origin.y + pc.size.height - pb.origin.y);
    let divider = cx.debug_bounds("divider-1").expect("the inner divider");
    let from = point(center(divider).x, center(divider).y - px(2.)); // inside B, within the grab area
    let to = point(from.x, from.y + px(100.));
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let w = window(&s.app, &ws, cx).0;
    let (outer2, inner2) = match w.layout.as_ref().unwrap() {
        Node::Split { ratio, after, .. } => match &**after {
            Node::Split { ratio: r, .. } => (*ratio, *r),
            _ => panic!("no inner split"),
        },
        _ => panic!("not split"),
    };
    let want = inner + 100. / (split_h as f64 - 1.);
    assert!((inner2 - want).abs() < 0.01, "inner ratio {inner} -> {inner2}, wanted {want}");
    assert_eq!(outer2, outer, "the outer split is untouched");
    assert_eq!(w.focus, focus, "a drag doesn't focus the pane under the divider");
    s.app.stop();
}

#[gpui::test]
fn click_focuses_and_dragging_a_divider_resizes(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let a = leaves(&window(&s.app, &ws, cx).0)[0].clone();
    cx.simulate_keystrokes("cmd-d");
    cx.run_until_parked();
    let b = window(&s.app, &ws, cx).0.focus.unwrap();

    let pane_a = cx.debug_bounds(Box::leak(format!("pane-{a}").into_boxed_str())).expect("pane a has bounds");
    cx.simulate_click(center(pane_a), Modifiers::none());
    assert_eq!(window(&s.app, &ws, cx).0.focus.as_deref(), Some(a.as_str()), "a click focuses the pane");

    let cols_before = term(&s.app, &a).size().1;
    let divider = cx.debug_bounds("divider-0").expect("the divider has bounds");
    let from = point(center(divider).x + px(2.), center(divider).y); // inside b, within the grab area
    let to = point(from.x + px(100.), from.y);
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let r = ratio(&window(&s.app, &ws, cx).0);
    assert!(r > 0.55 && r < 0.75, "dragging right raised the ratio: {r}");
    assert!(term(&s.app, &a).size().1 > cols_before, "the left pane grew");
    assert_eq!(window(&s.app, &ws, cx).0.focus.as_deref(), Some(a.as_str()), "a drag doesn't focus the pane under it");
    assert!(leaves(&window(&s.app, &ws, cx).0).contains(&b));
    s.app.stop();
}

#[gpui::test]
fn signature_follows_every_visible_pane_and_nothing_hidden(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let a = leaves(&window(&s.app, &ws, cx).0)[0].clone();
    cx.simulate_keystrokes("cmd-d");
    let b = window(&s.app, &ws, cx).0.focus.unwrap();
    let (ta, tb) = (term(&s.app, &a), term(&s.app, &b));
    settle(&ta);
    settle(&tb);
    let sig = |cx: &mut VisualTestContext| ws.read_with(cx, |ws, _| ws.signature());

    let before = sig(cx);
    cx.simulate_keystrokes("cmd-alt-left");
    assert_ne!(sig(cx), before, "a focus change redraws");
    cx.simulate_keystrokes("cmd-alt-right");
    let before = sig(cx);
    let sid = ws.read_with(cx, |ws, _| ws.session_id()).unwrap();
    let p = serde_json::json!({ "session_id": sid, "direction": "horizontal", "before_block_id": a, "after_block_id": b, "ratio": 0.3 });
    s.app.call("session.resize_pane", p).unwrap(); // from outside the app: it hears of it by an event
    assert!(changes(cx, &sig, before), "a layout change redraws");
    cx.run_until_parked();
    settle(&ta);
    settle(&tb);

    // a pushed screen redraws its own grid, not the window (GPUI phase 2, ADR R4)
    let grid = |cx: &mut VisualTestContext| ws.read_with(cx, |ws, _| ws.grid_renders().get(&a).copied().unwrap_or(0));
    let before = grid(cx);
    ta.write(b"echo unfocused\r").unwrap();
    wait_text(&ta, |x| has_line(x, "unfocused"));
    assert!(changes(cx, &grid, before), "output in a visible, unfocused pane redraws its grid");

    let before = sig(cx);
    cx.simulate_keystrokes("cmd-shift-enter"); // zooms b, hiding a
    assert_ne!(sig(cx), before, "zoom redraws");
    cx.run_until_parked();
    settle(&tb); // the zoom resized b, and its shell redraws for that
    let before = sig(cx);
    ta.write(b"echo behind\r").unwrap();
    wait_text(&ta, |x| has_line(x, "behind"));
    assert_eq!(sig(cx), before, "output in a pane hidden by a zoom doesn't redraw");

    cx.simulate_keystrokes("cmd-t");
    let t_new = term(&s.app, &window(&s.app, &ws, cx).0.focus.unwrap());
    settle(&t_new);
    settle(&ta);
    let before = sig(cx);
    ta.write(b"echo hidden\r").unwrap();
    wait_text(&ta, |x| has_line(x, "hidden"));
    assert_eq!(sig(cx), before, "output in a hidden tab doesn't redraw");
    s.app.stop();
}
