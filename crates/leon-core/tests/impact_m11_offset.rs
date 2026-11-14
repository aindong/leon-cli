//! M11 impact: counting pushed lines borrows the primary grid's display offset during a parse;
//! no client may ever see it moved, also when an alt-screen spell starts and ends in different
//! 4 KiB slices.

use alacritty_terminal::grid::Dimensions;
use leon_core::terminal::{Spec, Terminal};
use leon_core::theme::DARK;

fn shown(bytes: &[u8]) -> Terminal {
    let spec = Spec { command: vec!["/bin/sleep".into(), "60".into()], cwd: None, env: vec![] };
    Terminal::spawn_showing(spec, bytes, |_| {}).unwrap()
}

#[test]
fn the_view_is_never_left_scrolled() {
    let mut bytes: String = (0..100).map(|i| format!("p{i}\r\n")).collect();
    bytes.push_str("\x1b[?1049h");
    bytes.push_str(&"alt\r\n".repeat(2000)); // 10 KB: the spell spans slices
    let during = shown(bytes.as_bytes());
    assert_eq!(during.with_term(|t| t.grid().display_offset()), 0);
    assert!(during.frame(&DARK).text().contains("alt"), "the alt screen shows");
    bytes.push_str("\x1b[?1049l");
    bytes.push_str(&(100..200).map(|i| format!("p{i}\r\n")).collect::<String>());
    let t = shown(bytes.as_bytes());
    assert_eq!(t.with_term(|t| t.grid().display_offset()), 0, "back on the primary screen, at its bottom");
    let f = t.frame(&DARK);
    assert!(f.text().contains("p199"), "the newest lines are on screen:\n{}", f.text());
    assert_eq!(t.with_term(|t| t.screen_lines()), 24);
    let top = t.lines(f.view.scrolled, 1, &DARK).rows.pop().unwrap();
    assert_eq!(
        leon_core::screen::row_text(&top.spans),
        f.text().lines().next().unwrap(),
        "line scrolled is the screen's top row"
    );
}
