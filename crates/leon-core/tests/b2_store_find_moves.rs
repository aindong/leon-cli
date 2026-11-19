//! Scrollback B2, criterion 4: a find result keeps pointing at the same text after its line moves
//! from alacritty's rows into the store.

use std::time::{Duration, Instant};

use leon_core::search::Query;
use leon_core::terminal::{Spec, Terminal};

fn wait(t: &Terminal, what: &str) {
    let end = Instant::now() + Duration::from_secs(20);
    while !t.text(true).ends_with(what) {
        assert!(Instant::now() < end, "never printed {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn a_match_keeps_its_place_as_its_line_moves() {
    let spec = Spec {
        command: vec!["sh".into(), "-c".into(), "stty raw -echo; printf 'ready\\r\\n'; exec cat".into()],
        cwd: None,
        env: vec![],
    };
    let t = Terminal::spawn(spec, |_| {}).unwrap();
    wait(&t, "ready");
    let lines: String = (0..300).map(|i| format!("line {i}{}\r\n", if i % 50 == 7 { " marker" } else { "" })).collect();
    t.write(lines.as_bytes()).unwrap();
    wait(&t, "line 299");
    let q = Query { query: "marker".into(), regex: false, case_sensitive: true, limit: 100 };
    let before = t.find(&q).unwrap().matches;
    assert_eq!(before.len(), 6, "{before:?}");
    let more: String = (300..5000).map(|i| format!("line {i}\r\n")).collect();
    t.write(more.as_bytes()).unwrap();
    wait(&t, "line 4999");
    // a pause in output moves the history into the store
    let in_alacritty = |t: &Terminal| t.with_term(|t| alacritty_terminal::grid::Dimensions::history_size(t.grid()));
    let end = Instant::now() + Duration::from_secs(10);
    while in_alacritty(&t) > 1000 {
        assert!(Instant::now() < end, "the history never moved into the store");
        std::thread::sleep(Duration::from_millis(20));
    }
    let moved = t.scrolled() - in_alacritty(&t);
    assert!(before.iter().all(|m| m.end.line < moved), "the matched lines moved into the store");
    assert_eq!(t.find(&q).unwrap().matches, before, "the same matches at the same numbers");
}
