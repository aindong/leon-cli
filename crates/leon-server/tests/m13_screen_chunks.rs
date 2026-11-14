//! M13 criterion 9: the client reads at most 16 MiB a line, so the server never writes a screen
//! line past 4 MiB and a row: the worst screen a terminal can hold (500 × 1000 cells, each its own
//! colour, background and 16 combining marks) goes as several lines that rebuild it exactly.

use leon_core::screen::{self, BgRect, Frame, Span, Update};
use leon_server::screens::{encode, within_line, MAX_SCREEN_LINE};

fn worst() -> Frame {
    let (lines, cols) = (500, 1000);
    let mut f = Frame { lines, cols, rows: vec![Vec::new(); lines], ..Frame::default() };
    for line in 0..lines {
        for col in 0..cols {
            let n = (line * cols + col) as u32;
            let text: String = std::iter::once('e').chain(std::iter::repeat_n('\u{0301}', 16)).collect();
            let span = Span {
                col,
                cells: 1,
                text,
                fg: n & 0xff_ffff,
                bold: n.is_multiple_of(2),
                italic: false,
                underline: true,
                strikeout: false,
            };
            f.rows[line].push(span);
            f.backgrounds.push(BgRect { line, col, len: 1, color: (n * 7) & 0xff_ffff });
        }
    }
    f
}

#[test]
fn the_worst_screen_goes_in_bounded_lines_that_rebuild_it() {
    let server = worst();
    let update = screen::diff(None, &server).expect("a first screen");
    let lines = encode("block:x", update);
    assert!(lines.len() > 1, "a screen this big is split");
    let one_row = 1000 * 200; // a row's spans and backgrounds, generously
    let mut client = Frame::default();
    for (i, line) in lines.iter().enumerate() {
        assert!(line.len() <= MAX_SCREEN_LINE + one_row, "line {i} is {} bytes", line.len());
        assert!(MAX_SCREEN_LINE + one_row < 16 << 20, "every line fits the client's 16 MiB");
        assert_eq!(line.last(), Some(&b'\n'));
        let v: serde_json::Value = serde_json::from_slice(line).unwrap();
        assert_eq!(v["screen"]["block_id"], "block:x");
        assert_eq!(v["screen"]["full"], i == 0, "only the first line starts a new screen");
        let u: Update = serde_json::from_value(v["screen"].clone()).unwrap();
        screen::apply(&mut client, u);
    }
    assert_eq!(client, server, "the client's frame is the server's");
}

#[test]
fn a_small_update_is_one_line() {
    let mut f = Frame { lines: 2, cols: 10, rows: vec![Vec::new(); 2], ..Frame::default() };
    f.rows[0].push(Span {
        col: 0,
        cells: 2,
        text: "hi".into(),
        fg: 1,
        bold: false,
        italic: false,
        underline: false,
        strikeout: false,
    });
    let lines = encode("block:y", screen::diff(None, &f).unwrap());
    assert_eq!(lines.len(), 1);
}

#[test]
fn a_lines_reply_keeps_to_the_same_bound() {
    let rows = screen::diff(None, &worst()).unwrap().rows;
    let kept = within_line(rows.clone());
    assert!(!kept.is_empty() && kept.len() < rows.len(), "{} of {} rows", kept.len(), rows.len());
    assert_eq!(kept[..], rows[..kept.len()], "the first rows, in order");
    let bytes = serde_json::to_vec(&kept).unwrap().len();
    assert!(bytes <= MAX_SCREEN_LINE + 1000 * 200, "{bytes} bytes");
    assert_eq!(within_line(rows[..3].to_vec()).len(), 3, "a small reply is whole");
}
