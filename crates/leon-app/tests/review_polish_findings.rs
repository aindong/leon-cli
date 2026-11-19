//! The app polish review's findings (A2, A6): an agent's question stays in its notification when it
//! titles the Activity row; a caption with no spaces keeps its text when cut; Caps Lock on the `a`
//! key types what every other letter does.

mod m13_support;

use std::time::Instant;

use gpui::{Keystroke, Modifiers};
use leon_app::attention::{cut_words, Notifier, Tracker};
use leon_app::remote::Model;
use leon_core::program_status::{Kind, Record};
use m13_support::*;

fn agent(records: Vec<Record>) -> Model {
    let mut m = model(vec![("web", vec![Tab(None, "", records)])]);
    let b = &mut std::sync::Arc::make_mut(&mut m.sessions)[0].blocks[0];
    b.title = None; // the program set no OSC 0/2 title
    b.command = vec!["/usr/local/bin/fake-agent".into()];
    m
}

#[test]
fn a_blocked_agents_own_question_still_reaches_its_notification() {
    let mut t = Tracker::default();
    let now = Instant::now();
    t.update(&[(0, "This Mac", &agent(vec![]))], now); // the first look is quiet
    let asks = Record { title: Some("Run it on production now?".into()), ..blocked(Kind::Question) };
    let changes = t.update(&[(0, "This Mac", &agent(vec![asks]))], now);
    let to = changes.first().expect("the block turned blocked");
    let notice = Notifier::default().offer(to, now, false, false).expect("a blocked block posts");
    assert!(notice.body.contains("Run it on production now?"), "{notice:?}");
    assert_eq!(t.rows()[0].title, "Run it on production now?", "and it still titles the row");
}

#[test]
fn a_caption_without_spaces_keeps_its_text() {
    let what = format!("error · \u{2068}{}\u{2069}", "测试".repeat(40));
    let end = what.char_indices().nth(60).unwrap().0;
    let got = cut_words(&what, end);
    assert!(got.contains('测') && got.ends_with("…\u{2069}"), "the message vanished: {got}");
    let url = format!("error · \u{2068}https://example.com/{}\u{2069}", "a".repeat(200));
    let got = cut_words(&url, 80);
    assert!(got.contains("https://example.com/"), "{got}");
}

#[test]
fn caps_lock_a_stays_like_every_other_letter() {
    let ks = Keystroke { modifiers: Modifiers::none(), key: "a".into(), key_char: Some("a".into()) };
    // Caps Lock on: AppKit's characters say "A"; GPUI's key_char ignores Caps Lock for every key
    assert_eq!(leon_app::input::retext(ks.clone(), 0, "A"), ks, "caps lock changed only the a key");
}
