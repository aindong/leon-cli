//! Scanner: OSC 7501 is lifted out of the byte stream, everything else passes untouched, in any chunking.

use leon_core::program_status::{Piece, Scanner};

#[derive(Debug, PartialEq)]
enum Out {
    Bytes(Vec<u8>),
    Report(Vec<u8>),
    Prompt,
    Reset,
}

/// Feeds `chunks` in order and merges adjacent pass-through bytes, so chunking can't change the result.
fn scan(chunks: &[&[u8]]) -> Vec<Out> {
    let mut scanner = Scanner::default();
    let mut out = Vec::new();
    for chunk in chunks {
        scanner.feed(chunk, |piece| match piece {
            Piece::Bytes(b) => match out.last_mut() {
                Some(Out::Bytes(prev)) => prev.extend_from_slice(b),
                _ => out.push(Out::Bytes(b.to_vec())),
            },
            Piece::Report(p) => out.push(Out::Report(p.to_vec())),
            Piece::Prompt => out.push(Out::Prompt),
            Piece::Reset => out.push(Out::Reset),
        });
    }
    out
}

fn b(s: &str) -> Out {
    Out::Bytes(s.as_bytes().to_vec())
}

fn r(s: &str) -> Out {
    Out::Report(s.as_bytes().to_vec())
}

#[test]
fn scanner_table_in_every_chunking() {
    let cases: Vec<(&str, &str, Vec<Out>)> = vec![
        ("plain text", "hello", vec![b("hello")]),
        ("utf-8 text", "héllo 👋", vec![b("héllo 👋")]),
        ("BEL report", "a\x1b]7501;state=idle\x07b", vec![b("a"), r("state=idle"), b("b")]),
        ("ST report", "a\x1b]7501;state=idle\x1b\\b", vec![b("a"), r("state=idle"), b("b")]),
        ("empty payload", "\x1b]7501;\x07", vec![r("")]),
        ("query", "\x1b]7501;?\x07", vec![r("?")]),
        ("two reports", "\x1b]7501;x\x07\x1b]7501;y\x1b\\", vec![r("x"), r("y")]),
        ("other OSC", "\x1b]0;title\x07", vec![b("\x1b]0;title\x07")]),
        ("OSC 75010", "\x1b]75010;x\x07", vec![b("\x1b]75010;x\x07")]),
        ("OSC 750", "\x1b]750;x\x07", vec![b("\x1b]750;x\x07")]),
        ("7501 without ;", "\x1b]7501\x07z", vec![b("\x1b]7501\x07z")]),
        ("CSI", "\x1b[31mred\x1b[0m", vec![b("\x1b[31mred\x1b[0m")]),
        ("bare ST", "\x1b\\", vec![b("\x1b\\")]),
        ("RIS", "x\x1bcy", vec![b("x\x1bc"), Out::Reset, b("y")]),
        ("prompt BEL", "\x1b]133;A\x07$ ", vec![b("\x1b]133;A\x07"), Out::Prompt, b("$ ")]),
        ("prompt ST + params", "\x1b]133;A;aid=1\x1b\\", vec![b("\x1b]133;A;aid=1\x1b\\"), Out::Prompt]),
        ("prompt with shell params", "\x1b]133;A;aid=1;cl=m\x07", vec![b("\x1b]133;A;aid=1;cl=m\x07"), Out::Prompt]),
        ("DECSTR is no reset", "\x1b[!p", vec![b("\x1b[!p")]),
        ("alt screen is no reset", "\x1b[?1049h\x1b[?1049l", vec![b("\x1b[?1049h\x1b[?1049l")]),
        ("133 B is no prompt", "\x1b]133;B\x07", vec![b("\x1b]133;B\x07")]),
        ("133 AB is no prompt", "\x1b]133;AB\x07", vec![b("\x1b]133;AB\x07")]),
        ("1330 is no prompt", "\x1b]1330;A\x07", vec![b("\x1b]1330;A\x07")]),
        ("abort by ESC [", "\x1b]7501;state=idle\x1b[31mX", vec![b("\x1b[31mX")]),
        ("abort by CAN", "\x1b]7501;state=idle\x18rest", vec![b("\x18rest")]),
        ("abort by SUB", "\x1b]7501;state=idle\x1arest", vec![b("\x1arest")]),
        ("abort then report", "\x1b]7501;a\x1b]7501;b\x07", vec![r("b")]),
        ("abort then RIS", "\x1b]7501;a\x1bc", vec![b("\x1bc"), Out::Reset]),
        ("ESC ends other OSC", "\x1b]0;t\x1b]7501;state=idle\x07", vec![b("\x1b]0;t"), r("state=idle")]),
        ("double ESC", "\x1b\x1b]7501;x\x07", vec![b("\x1b"), r("x")]),
        ("prefix cut by ESC", "\x1b]75\x1b]7501;x\x07", vec![b("\x1b]75"), r("x")]),
        ("raw C1 OSC is text", "\u{9d}7501;x\x07", vec![b("\u{9d}7501;x\x07")]),
    ];
    for (name, input, want) in cases {
        let bytes = input.as_bytes();
        assert_eq!(scan(&[bytes]), want, "{name}: one read");
        let singles: Vec<&[u8]> = bytes.chunks(1).collect();
        assert_eq!(scan(&singles), want, "{name}: byte by byte");
        for cut in 0..=bytes.len() {
            let (x, y) = bytes.split_at(cut);
            assert_eq!(scan(&[x, y]), want, "{name}: split at {cut}");
        }
    }
}

#[test]
fn unterminated_report_is_held_not_passed() {
    assert_eq!(scan(&[b"ok\x1b]7501;state=working"]), vec![b("ok")]);
}

#[test]
fn oversized_report_is_swallowed_to_its_terminator() {
    let big = format!("\x1b]7501;{}\x07after", "a".repeat(5000));
    assert_eq!(scan(&[big.as_bytes()]), vec![b("after")]);
}
