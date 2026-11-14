//! Review of M13: a notice carries nothing hidden (OSC 7501's whole rule, and no line breaks) even
//! from names set over the API or by a remote server, and a long caption never pushes out the state.

use std::time::Instant;

use leon_app::attention::{clean, Notifier, Transition};
use leon_app::status::Indicator;
use leon_core::program_status::Kind;

const HIDDEN: [char; 8] =
    ['\u{2028}', '\u{2029}', '\u{e0041}', '\u{e007f}', '\u{3164}', '\u{115f}', '\u{fe01}', '\u{34f}'];

fn blocked(place: &str, caption: &str) -> Transition {
    Transition {
        host: 0,
        host_label: "This Mac".into(),
        session_id: "session:web".into(),
        block_id: "b1".into(),
        place: place.into(),
        to: Indicator::Blocked(Some(Kind::Permission)),
        caption: Some(caption.into()),
        worked: None,
    }
}

#[test]
fn clean_drops_every_hidden_character() {
    for c in HIDDEN {
        let out = clean(&format!("a{c}b"));
        assert!(!out.contains(c), "kept U+{:04X}: {out:?}", c as u32);
        assert!(out.starts_with('a') && out.ends_with('b'), "{out:?}");
    }
}

#[test]
fn a_name_with_a_line_separator_and_tags_reaches_no_banner() {
    let mut n = Notifier::default();
    let t = blocked("This Mac › web\u{2028}Approve now \u{e0041}\u{e0042} › claude", "Allow edit?");
    let notice = n.offer(&t, Instant::now(), false, false).expect("a blocked block posts");
    for c in HIDDEN {
        assert!(!notice.title.contains(c), "kept U+{:04X} in {:?}", c as u32, notice.title);
    }
}

#[test]
fn a_long_caption_leaves_room_for_the_state() {
    let mut n = Notifier::default();
    let notice = n.offer(&blocked("This Mac › web › claude", &"x".repeat(500)), Instant::now(), false, false).unwrap();
    assert!(notice.body.ends_with("(blocked: permission)"), "{:?}", notice.body);
    assert!(notice.body.chars().count() <= 120, "{} chars", notice.body.chars().count());
}
