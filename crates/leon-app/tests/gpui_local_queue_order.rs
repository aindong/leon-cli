//! GPUI phase 5 (ADR R8): This Mac's calls go through one ordered queue, as a host's link does.
//! A split that first asks its pane's working directory is one job on that queue, so a focus move
//! pressed right after it can't overtake it. A fake server logs the order the calls arrive in.
mod fake_support;
mod m10b_support;

use fake_support::{open_fake, serve, Answer};
use gpui::{ClipboardItem, Keystroke, TestAppContext};
use leon_app::view::Paste;
use m10b_support::*;

#[gpui::test]
fn calls_reach_this_mac_in_the_order_sent(cx: &mut TestAppContext) {
    let fake = serve(Answer::All);
    let (ws, cx) = open_fake(cx, &fake);
    cx.write_to_clipboard(ClipboardItem::new_string("pasted".into()));
    // all sent before the queue runs once (each step alone would let it run in between, and a
    // call sent past the queue would still look in order)
    let key = |cx: &mut gpui::VisualTestContext, k: &str| {
        cx.update(|window, cx| assert!(window.dispatch_keystroke(Keystroke::parse(k).unwrap(), cx), "{k} handled"))
    };
    let action = |cx: &mut gpui::VisualTestContext, name: &str| {
        cx.update(|window, cx| ws.update(cx, |ws, cx| ws.run_action(name, None, "test", window, cx)))
    };
    key(cx, "a");
    cx.update(|window, cx| window.dispatch_action(Box::new(Paste), cx));
    action(cx, "pane.split.right");
    action(cx, "pane.focus.left");
    key(cx, "b");
    cx.run_until_parked();
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
