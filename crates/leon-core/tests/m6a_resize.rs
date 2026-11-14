//! Criterion 3: a terminal resizes its PTY and grid, and counts changes for redraws.

use std::time::{Duration, Instant};

use leon_core::terminal::{Spec, Terminal};

fn until(t: &Terminal, what: &str) -> String {
    let end = Instant::now() + Duration::from_secs(10);
    loop {
        let text = t.text(true);
        if text.contains(what) || Instant::now() > end {
            return text;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn resize_reaches_the_program() {
    let spec = Spec { command: vec!["sh".into()], cwd: None, env: vec![] };
    let t = Terminal::spawn(spec, |_| {}).unwrap();
    t.resize(30, 100).unwrap();
    assert_eq!(t.size(), (30, 100));
    let before = t.generation();
    t.write(b"stty size\r").unwrap();
    let text = until(&t, "30 100");
    assert!(text.lines().any(|l| l.trim() == "30 100"), "{text}");
    assert!(t.generation() > before, "output bumps the change counter");
    t.with_term(|term| {
        use alacritty_terminal::grid::Dimensions;
        assert_eq!((term.screen_lines(), term.columns()), (30, 100));
    });
}
