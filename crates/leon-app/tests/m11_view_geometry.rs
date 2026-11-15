//! M11: the scrolled view's rows and which cells a selection covers (`leon_app::scrollback`).

use std::collections::HashMap;

use leon_app::render::{Cursor, Frame, Row, Span, View};
use leon_app::scrollback::{clamp, compose, covered, text_of};
use leon_core::search::Pos;

fn span(col: usize, text: &str) -> Span {
    let cells = text.chars().count();
    Span { col, cells, text: text.into(), fg: 0, bold: false, italic: false, underline: false, strikeout: false }
}

/// A 3-row screen whose top row is line 100, with 50 lines of history.
fn live() -> Frame {
    let rows = vec![vec![span(0, "s100")], vec![span(0, "s101")], vec![span(0, "s102")]];
    let cursor = Cursor {
        line: 2,
        col: 0,
        shape: alacritty_terminal::vte::ansi::CursorShape::Block,
        wide: false,
        text: " ".into(),
    };
    let view = View { scrolled: 100, history: 50, ..View::default() };
    Frame { lines: 3, cols: 10, rows, cursor: Some(cursor), view, ..Frame::default() }
}

#[test]
fn a_scrolled_view_takes_history_rows_then_screen_rows() {
    let cache: HashMap<usize, Row> =
        [(99, Row { line: 99, spans: vec![span(0, "h99")], backgrounds: vec![] })].into_iter().collect();
    let c = compose(&live(), Some(99), &cache);
    assert_eq!(c.frame.text(), "h99\ns100\ns101\n");
    assert!(c.missing.is_empty());
    assert_eq!(c.frame.cursor, None, "the cursor's row is below the view");
    let c = compose(&live(), Some(97), &cache);
    assert_eq!(c.missing, vec![97, 98], "rows to fetch");
    assert_eq!(c.frame.cursor, None);
    let c = compose(&live(), None, &cache);
    assert_eq!(c.frame.cursor.map(|c| c.line), Some(2), "at the bottom the cursor is where it is");
}

#[test]
fn the_top_stays_inside_the_scrollback() {
    assert_eq!(clamp(&live(), Some(10), false), Some(50), "the oldest kept line");
    assert_eq!(clamp(&live(), Some(100), false), None, "the bottom follows the output");
    assert_eq!(clamp(&live(), Some(500), false), None);
    assert_eq!(clamp(&live(), None, true), Some(100), "a pinned view (copy mode) holds the bottom");
    assert_eq!(clamp(&live(), Some(500), true), Some(100));
}

#[test]
fn covered_columns_for_lines_and_rectangles() {
    let (a, b) = (Pos { line: 5, col: 3 }, Pos { line: 7, col: 2 });
    assert_eq!(covered(a, b, false, 5, 10), Some((3, 9)), "the first line to its end");
    assert_eq!(covered(a, b, false, 6, 10), Some((0, 9)), "a middle line whole");
    assert_eq!(covered(a, b, false, 7, 10), Some((0, 2)));
    assert_eq!(covered(a, b, false, 8, 10), None);
    assert_eq!(covered(a, b, true, 6, 10), Some((2, 3)), "a rectangle: the same columns on each line");
}

#[test]
fn reading_columns_back_keeps_wide_characters_whole() {
    let row = vec![span(0, "a"), Span { cells: 2, ..span(1, "中") }, span(3, "b")];
    assert_eq!(text_of(&row, 10, 0, 3), "a中b");
    assert_eq!(text_of(&row, 10, 1, 2), "中");
    assert_eq!(text_of(&row, 10, 3, 9), "b", "trailing blanks dropped");
}
