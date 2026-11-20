//! Scrollback B1, criterion 2: styles survive the move into the store. The same styled lines are
//! read from a terminal where they are still alacritty's rows and from one where later output
//! moved them into the store: the rows' JSON must be the same bytes.

use alacritty_terminal::grid::Dimensions;
use leon_core::terminal::{Spec, Terminal};
use leon_core::theme::{DARK, LIGHT};

fn shown(bytes: &[u8]) -> Terminal {
    let spec = Spec { command: vec!["/bin/sleep".into(), "60".into()], cwd: None, env: vec![] };
    Terminal::spawn_showing(spec, bytes, |_| {}).unwrap()
}

/// Every style a cell can carry, each on lines of its own and mixed on one.
fn styled() -> String {
    [
        "plain text, then \x1b[1mbold\x1b[0m \x1b[3mitalic\x1b[0m \x1b[4munderline\x1b[0m \x1b[9mstrike\x1b[0m",
        "\x1b[2mdim\x1b[0m \x1b[7minverse\x1b[0m \x1b[8mhidden\x1b[0m \x1b[4:3mcurly\x1b[0m \x1b[21mdouble\x1b[0m",
        "\x1b[31mred\x1b[0m \x1b[92mbright green\x1b[0m \x1b[38;5;208m256 orange\x1b[0m \x1b[38;2;1;2;3mtrue\x1b[0m",
        "\x1b[41m red bg \x1b[0m \x1b[48;5;27mblue 256\x1b[0m \x1b[48;2;250;240;230m rgb bg \x1b[0m trailing bg\x1b[44m   \x1b[0m",
        "wide: 中文字 and 🎉 emoji, ｆｕｌｌ width",
        "marks: e\u{301} a\u{308}\u{304} n\u{303} and a stack x\u{301}\u{302}\u{303}\u{304}",
        "\x1b]8;;https://example.com/a\x1b\\a link\x1b]8;;\x1b\\ and \x1b]8;id=x;file:///tmp/b\x1b\\another\x1b]8;;\x1b\\",
        "\x1b[1;3;4;31;43mall at once\x1b[22;23;24;39;49m back \x1b[5mblink\x1b[25m",
        "tabs:\there\tand\tthere",
        "\x1b]4;1;rgb:12/34/56\x1b\\\x1b[31mpalette red\x1b[0m",
    ]
    .iter()
    .map(|l| format!("{l}\r\n"))
    .collect()
}

fn filler(n: usize) -> String {
    (0..n).map(|i| format!("filler {i}\r\n")).collect()
}

/// Rows of the styled lines (numbers 0.. while kept) as JSON, under `theme`.
fn json(t: &Terminal, count: usize, theme: &leon_core::theme::Theme) -> String {
    serde_json::to_string(&t.lines(0, count, theme).rows).unwrap()
}

#[test]
fn styled_rows_read_the_same_from_the_store() {
    let n = styled().lines().count();
    // 30 lines of filler push the styled lines into history, still alacritty's
    let near = shown((styled() + &filler(30)).as_bytes());
    let far = shown((styled() + &filler(3000)).as_bytes());
    let l = far.lines(0, 0, &DARK);
    let alacritty = far.with_term(|t| t.grid().history_size());
    assert!(l.scrolled - l.history == 0 && alacritty < l.history - n, "the styled lines are in the store");
    for theme in [&DARK, &LIGHT] {
        let (a, b) = (json(&near, n, theme), json(&far, n, theme));
        assert!(a.len() > 1000, "the styled rows were read: {a}");
        assert_eq!(a, b, "the same spans and backgrounds from the store");
    }
}

#[test]
fn a_wrapped_wide_character_and_long_lines_read_the_same() {
    // 79 columns then a wide char: it wraps, leaving a leading spacer
    let mut s = "x".repeat(79) + "中\r\n";
    s += &format!("{}\r\n", "\x1b[32mlong green \x1b[0m".repeat(30));
    let n = 2 + 5; // the wrapped lines' rows
    let near = shown((s.clone() + &filler(30)).as_bytes());
    let far = shown((s + &filler(3000)).as_bytes());
    assert_eq!(json(&near, n, &DARK), json(&far, n, &DARK));
    assert_eq!(near.text(true).lines().take(3).collect::<Vec<_>>(), far.text(true).lines().take(3).collect::<Vec<_>>());
}
