//! Scrollback B1, criterion 3: `capture`'s text (`Terminal::text`) with most of the history in the
//! store is byte for byte what alacritty alone, keeping 10,000 lines, gives for the same output.

use alacritty_terminal::event::VoidListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Point};
use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::vte::ansi::Processor;
use leon_core::terminal::{Spec, Terminal};
use leon_core::theme::DARK;

struct Size;

impl Dimensions for Size {
    fn total_lines(&self) -> usize {
        24
    }
    fn screen_lines(&self) -> usize {
        24
    }
    fn columns(&self) -> usize {
        80
    }
}

/// What `Terminal::text` gave before the store: alacritty's own text of 10,000 lines.
fn today(bytes: &[u8], trim: bool) -> String {
    let mut term = Term::new(Config { scrolling_history: 10_000, ..Config::default() }, &Size, VoidListener);
    let mut parser: Processor = Processor::new();
    parser.advance(&mut term, bytes);
    let start = Point::new(term.topmost_line(), Column(0));
    let end = Point::new(term.bottommost_line(), term.last_column());
    let text = term.bounds_to_string(start, end);
    if !trim {
        return text;
    }
    let lines: Vec<&str> = text.lines().map(str::trim_end).collect();
    let keep = lines.iter().rposition(|l| !l.is_empty()).map_or(0, |i| i + 1);
    lines[..keep].join("\n")
}

/// 12,000 lines of everything `text` treats specially.
fn output() -> Vec<u8> {
    let mut s = String::new();
    for i in 0..12_000 {
        s += &match i % 10 {
            0 => format!("plain {i}"),
            1 => format!("{} {i}", "wrapped across rows ".repeat(9)),
            2 => format!("tab\t{i}\tcols\tx"),
            3 => format!("\x1b[1;31mred {i}\x1b[0m and \x1b[48;5;22mbg\x1b[0m   "),
            4 => format!("wide 中文 {i} 🎉"),
            5 => format!("marks e\u{301} {i} a\u{308}"),
            6 => format!("{}中 {i}", "w".repeat(79)), // a wide char wrapped to the next row
            7 => String::new(),
            8 => format!("trailing spaces {i}      "),
            _ => format!("\x1b]8;;https://example.com/{i}\x1b\\link {i}\x1b]8;;\x1b\\"),
        };
        s += "\r\n";
    }
    s += "the last line";
    s.into_bytes()
}

#[test]
fn capture_of_ten_thousand_lines_is_todays_byte_for_byte() {
    let bytes = output();
    let spec = Spec { command: vec!["/bin/sleep".into(), "60".into()], cwd: None, env: vec![] };
    let t = Terminal::spawn_showing(spec, &bytes, |_| {}).unwrap();
    let l = t.lines(0, 0, &DARK);
    let alacritty = t.with_term(|t| t.grid().history_size());
    assert_eq!(l.history, 10_000);
    assert!(alacritty <= 1000, "most of the history is in the store ({alacritty} rows in alacritty)");
    for trim in [false, true] {
        let (got, want) = (t.text(trim), today(&bytes, trim));
        assert!(want.len() > 300_000, "the reference holds the history ({} bytes)", want.len());
        if got != want {
            let at = got.bytes().zip(want.bytes()).position(|(a, b)| a != b).unwrap_or(got.len().min(want.len()));
            let near = |s: &str| s.get(at.saturating_sub(80)..(at + 80).min(s.len())).unwrap_or("").to_string();
            panic!("trim={trim}: differs at byte {at}\n got: {:?}\nwant: {:?}", near(&got), near(&want));
        }
    }
}
