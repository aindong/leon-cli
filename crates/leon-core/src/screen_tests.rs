use alacritty_terminal::vte::ansi::CursorShape;

use super::*;

fn span(col: usize, text: &str, fg: u32) -> Span {
    let cells = text.chars().count();
    Span { col, cells, text: text.into(), fg, bold: false, italic: false, underline: false, strikeout: false }
}

/// A `lines`×`cols` frame with `text[i]` on line i and a background on line 0.
fn frame(lines: usize, cols: usize, text: &[&str], cursor_line: usize) -> Frame {
    let mut rows = vec![Vec::new(); lines];
    for (i, t) in text.iter().enumerate() {
        rows[i] = vec![span(0, t, 0xffffff)];
    }
    let cursor = Cursor { line: cursor_line, col: 1, shape: CursorShape::Beam, wide: false, text: " ".into() };
    let backgrounds = vec![BgRect { line: 0, col: 2, len: 3, color: 0x112233 }];
    Frame {
        lines,
        cols,
        rows,
        backgrounds,
        cursor: Some(cursor),
        app_cursor: false,
        bracketed_paste: true,
        ..Frame::default()
    }
}

fn through_the_wire(u: Update) -> Update {
    serde_json::from_str(&serde_json::to_string(&u).unwrap()).unwrap()
}

#[test]
fn a_first_screen_is_whole_and_applies_to_nothing() {
    let b = frame(3, 10, &["one", "two"], 1);
    let u = diff(None, &b).unwrap();
    assert!(u.full);
    assert_eq!(u.rows.len(), 3);
    let mut got = Frame::default();
    apply(&mut got, through_the_wire(u));
    assert_eq!(got, b);
}

#[test]
fn only_changed_rows_go_and_apply_gives_the_new_screen() {
    let a = frame(4, 10, &["one", "two", "three"], 1);
    let mut b = frame(4, 10, &["one", "TWO", "three"], 1);
    b.backgrounds.push(BgRect { line: 3, col: 0, len: 1, color: 0x445566 });
    let u = diff(Some(&a), &b).unwrap();
    assert!(!u.full);
    assert_eq!(u.rows.iter().map(|r| r.line).collect::<Vec<_>>(), vec![1, 3], "{u:?}");
    let mut got = a.clone();
    apply(&mut got, through_the_wire(u));
    assert_eq!(got, b);
}

#[test]
fn a_background_leaving_a_row_is_a_change() {
    let a = frame(2, 10, &["x"], 0);
    let mut b = a.clone();
    b.backgrounds.clear();
    let mut got = a.clone();
    apply(&mut got, diff(Some(&a), &b).unwrap());
    assert_eq!(got, b);
}

#[test]
fn the_cursor_or_a_mode_alone_is_an_update_and_nothing_is_none() {
    let a = frame(3, 10, &["one"], 0);
    assert_eq!(diff(Some(&a), &a), None);
    let moved = frame(3, 10, &["one"], 2);
    let u = diff(Some(&a), &moved).unwrap();
    assert!(u.rows.is_empty() && !u.full);
    let mut keys = a.clone();
    keys.app_cursor = true;
    let mut got = a.clone();
    apply(&mut got, diff(Some(&a), &keys).unwrap());
    assert!(got.app_cursor);
}

#[test]
fn a_new_size_is_a_full_screen() {
    let a = frame(3, 10, &["one", "two", "three"], 0);
    for b in [frame(5, 10, &["one"], 4), frame(2, 10, &["x"], 0), frame(3, 20, &["one", "two", "three"], 0)] {
        let u = diff(Some(&a), &b).unwrap();
        assert!(u.full && u.rows.len() == b.lines, "{u:?}");
        let mut got = a.clone();
        apply(&mut got, u);
        assert_eq!(got, b);
    }
}

#[test]
fn rows_past_the_screen_are_ignored() {
    let a = frame(2, 10, &["one"], 0);
    let mut u = diff(None, &a).unwrap();
    u.full = false;
    u.rows.push(Row { line: 99, spans: vec![span(0, "x", 0)], backgrounds: vec![] });
    let mut got = a.clone();
    apply(&mut got, u);
    assert_eq!(got.rows.len(), 2);
}

#[test]
fn text_places_spans_at_their_columns() {
    let mut f = frame(2, 10, &[], 0);
    f.rows[0] = vec![span(0, "ab", 1), span(4, "c", 2)];
    assert_eq!(f.text(), "ab  c\n\n");
}
