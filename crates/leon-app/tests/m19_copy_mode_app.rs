//! M19 criterion 8 (.agent/plans/2026-11-15-m19-tmux-migrants.md): copy mode in the app: ⌘⇧C,
//! vi motions over the scrollback, v and V selections, y to the clipboard, / and ? with n and N,
//! g and G, Esc and q; no key reaches the program meanwhile.
mod m11_support;

use gpui::TestAppContext;
use m11_support::*;

/// 300 numbered lines, the line copy mode works on, then `cat`, which echoes anything typed.
const SCRIPT: &str = "seq 1 300; echo 'alpha beta gamma'; exec cat";

fn cursor(ws: &gpui::Entity<leon_app::view::Workspace>, cx: &mut gpui::VisualTestContext, id: &str) -> Option<String> {
    cx.run_until_parked();
    ws.read_with(cx, |ws, _| ws.copy_cursor(id)).map(|(_, text)| text)
}

fn keys(cx: &mut gpui::VisualTestContext, k: &str) {
    cx.simulate_keystrokes(k);
    cx.run_until_parked();
}

#[gpui::test]
fn motions_selections_and_yank(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let id = tab_running(&s, &ws, cx, SCRIPT, "alpha beta gamma");
    assert_eq!(ws.read_with(cx, |ws, _| ws.copy_cursor(&id)), None);

    keys(cx, "cmd-shift-c");
    let (at, _) = ws.read_with(cx, |ws, _| ws.copy_cursor(&id)).expect("⌘⇧C enters copy mode");
    assert_eq!(at.col, 0, "it starts at the terminal's cursor");
    let drawn = ws.read_with(cx, |ws, _| ws.drawn_text(&id)).unwrap();
    assert!(drawn.lines().next().unwrap().trim_end().ends_with("COPY"), "the badge: {drawn}");
    let alpha = row_of(&ws, cx, &id, "alpha beta gamma");
    assert_eq!(ws.read_with(cx, |ws, _| ws.copy_mark(&id)), Some((alpha + 1, 0)), "the cursor is outlined");

    keys(cx, "k w");
    assert_eq!(cursor(&ws, cx, &id).as_deref(), Some("alpha beta gamma"));
    assert_eq!(ws.read_with(cx, |ws, _| ws.copy_cursor(&id)).unwrap().0.col, 6, "on the b");
    assert_eq!(ws.read_with(cx, |ws, _| ws.copy_mark(&id)), Some((alpha, 6)), "the outline follows");
    keys(cx, "v y");
    assert_eq!(clipboard(cx).as_deref(), Some("b"), "v y copies the cell under the cursor");
    assert_eq!(ws.read_with(cx, |ws, _| ws.copy_cursor(&id)), None, "y exits");

    keys(cx, "cmd-shift-c k w v l y");
    assert_eq!(clipboard(cx).as_deref(), Some("be"));
    keys(cx, "cmd-shift-c k w v $ y");
    assert_eq!(clipboard(cx).as_deref(), Some("beta gamma"), "to the last non-blank");
    keys(cx, "cmd-shift-c k shift-v y");
    assert_eq!(clipboard(cx).map(|t| t.trim_end().to_string()).as_deref(), Some("alpha beta gamma"));
    keys(cx, "cmd-shift-c k k 0 v j $ y");
    assert_eq!(clipboard(cx).as_deref(), Some("300\nalpha beta gamma"), "across lines");

    // nothing typed in copy mode reached cat, which would have echoed it
    let screen = ws.read_with(cx, |ws, _| ws.screen_text(&id)).unwrap();
    let after: Vec<&str> = screen.lines().skip_while(|l| !l.starts_with("alpha")).skip(1).collect();
    assert!(after.iter().all(|l| l.trim().is_empty()), "keys leaked into the pane: {after:?}");
    s.app.stop();
}

#[gpui::test]
fn search_steps_and_the_ends_of_the_scrollback(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let id = tab_running(&s, &ws, cx, SCRIPT, "alpha beta gamma");
    keys(cx, "cmd-shift-c");

    keys(cx, "? 2 5 enter");
    assert_eq!(cursor(&ws, cx, &id).as_deref(), Some("259"), "the nearest match above");
    keys(cx, "n");
    assert_eq!(cursor(&ws, cx, &id).as_deref(), Some("258"), "n keeps going up");
    keys(cx, "shift-n");
    assert_eq!(cursor(&ws, cx, &id).as_deref(), Some("259"), "N goes back down");
    keys(cx, "/ 1 2 5 enter");
    assert_eq!(cursor(&ws, cx, &id).as_deref(), Some("125"), "/ searches down, wrapping round");
    assert!(view_lines(&ws, cx, &id).contains(&"125".to_string()), "the view follows the cursor");

    keys(cx, "g");
    assert_eq!(cursor(&ws, cx, &id).as_deref(), Some("1"), "g: the oldest line");
    assert!(ws.read_with(cx, |ws, _| ws.view_top(&id)).is_some());
    keys(cx, "shift-g");
    assert!(ws.read_with(cx, |ws, _| ws.view_top(&id)).is_some(), "G: back at the bottom, held still");
    assert!(view_lines(&ws, cx, &id).contains(&"alpha beta gamma".to_string()), "the bottom is in view");
    let bottom = ws.read_with(cx, |ws, _| ws.copy_cursor(&id)).unwrap().0.line;
    keys(cx, "ctrl-u");
    let up = ws.read_with(cx, |ws, _| ws.copy_cursor(&id)).unwrap().0.line;
    let rows = view_lines(&ws, cx, &id).len();
    assert_eq!(bottom - up, rows / 2, "ctrl+u: half a page");
    keys(cx, "ctrl-d");
    assert_eq!(ws.read_with(cx, |ws, _| ws.copy_cursor(&id)).unwrap().0.line, bottom);

    keys(cx, "/ z z z escape");
    assert!(ws.read_with(cx, |ws, _| ws.copy_cursor(&id)).is_some(), "Esc drops the prompt, not copy mode");
    keys(cx, "escape");
    assert_eq!(ws.read_with(cx, |ws, _| ws.copy_cursor(&id)), None, "Esc exits");
    keys(cx, "cmd-shift-c q");
    assert_eq!(ws.read_with(cx, |ws, _| ws.copy_cursor(&id)), None, "q exits");
    s.app.stop();
}
