//! GPUI phase 5 (ADR R8): This Mac's calls go through one ordered queue, as a host's link does.
//! A split that first asks its pane's working directory is one job on that queue, so a focus move
//! pressed right after it can't overtake it. A fake server logs the order the calls arrive in.
mod fake_support;
mod m10b_support;

use fake_support::{open_fake, serve, Answer};
use gpui::{ClipboardItem, TestAppContext};
use leon_app::view::Paste;
use m10b_support::*;

#[gpui::test]
fn calls_reach_this_mac_in_the_order_sent(cx: &mut TestAppContext) {
    let fake = serve(Answer::All);
    let (ws, cx) = open_fake(cx, &fake);
    cx.write_to_clipboard(ClipboardItem::new_string("pasted".into()));
    cx.simulate_keystrokes("a");
    cx.dispatch_action(Paste);
    act(&ws, cx, "pane.split.right");
    act(&ws, cx, "pane.focus.left");
    cx.simulate_keystrokes("b");
    let wanted = [
        "dev.leon.terminal.write",
        "block.event paste",
        "dev.leon.terminal.pwd",
        "session.new_split",
        "session.focus_direction",
        "dev.leon.terminal.write",
    ];
    let seen = || -> Vec<String> {
        let log = fake.log.lock().unwrap().clone();
        log.into_iter().filter(|m| wanted.contains(&m.as_str())).collect()
    };
    let all = eventually(|| seen().len() >= wanted.len());
    assert!(all, "every call arrived: {:?}", seen());
    assert_eq!(seen(), wanted, "in the order sent");
}
