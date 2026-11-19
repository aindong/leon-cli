//! A pane made taller fills its new rows from history whether that history is still alacritty's or
//! already in leon's store (docs/architecture/scrollback.md, "Resize"): the same output, resized
//! alike, reads the same as alacritty alone keeping the whole history: text, styles, numbers.

use alacritty_terminal::event::VoidListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::vte::ansi::Processor;
use leon_core::screen::{self, Row};
use leon_core::search::{self, Query};
use leon_core::terminal::{Spec, Terminal};
use leon_core::theme::DARK;

struct Size(usize, usize);

impl Dimensions for Size {
    fn total_lines(&self) -> usize {
        self.0
    }
    fn screen_lines(&self) -> usize {
        self.0
    }
    fn columns(&self) -> usize {
        self.1
    }
}

/// `short` lines (at most 50 cells, so no width here re-wraps them) in every style a stored row
/// keeps, then `long` lines of 100 characters, which wrap at 80 and 60 and join at 120.
fn output(short: usize, long: usize) -> Vec<u8> {
    let mut s = String::new();
    for i in 0..short {
        s += &match i % 6 {
            0 => format!("plain {i} \x1b[1mbold\x1b[0m \x1b[3;4mitalic under\x1b[0m"),
            1 => format!("\x1b[31mred {i}\x1b[0m \x1b[38;2;1;2;3mtrue\x1b[0m \x1b[44m blue bg \x1b[0m"),
            2 => format!("wide 中文{i} and e\u{301} mark"),
            3 => format!("\x1b]8;;https://example.com/{i}\x1b\\link {i}\x1b]8;;\x1b\\ after"),
            4 => format!("needle {i} trailing bg\x1b[42m   \x1b[0m"),
            _ => format!("tab\t{i}\tthere"),
        };
        s += "\r\n";
    }
    for i in 0..long {
        s += &format!("long-{i:03}-{}\r\n", "z".repeat(91));
    }
    s.into_bytes()
}

fn spec() -> Spec {
    Spec { command: vec!["/bin/sleep".into(), "60".into()], cwd: None, env: vec![] }
}

/// leon at `rows` x 80, shown `bytes`: past 1,000 rows of history, all of it is in the store.
fn leon(bytes: &[u8], rows: u16) -> Terminal {
    Terminal::spawn_sized(spec(), bytes, Some((rows, 80)), 10_000, |_| {}).unwrap()
}

/// Rows of history alacritty holds; the older ones are in the store.
fn held(t: &Terminal) -> usize {
    t.with_term(|t| t.grid().history_size())
}

/// alacritty alone at `rows` x 80, keeping the whole history.
fn plain(bytes: &[u8], rows: usize) -> Term<VoidListener> {
    let mut term = Term::new(Config { scrolling_history: 10_000, ..Config::default() }, &Size(rows, 80), VoidListener);
    let mut parser: Processor = Processor::new();
    parser.advance(&mut term, bytes);
    term
}

/// Every row leon keeps, history and screen, by number.
fn rows(t: &Terminal) -> Vec<Row> {
    let l = t.lines(0, 0, &DARK);
    let (from, end) = (l.scrolled - l.history, l.scrolled + t.size().0 as usize);
    (from..end).step_by(1000).flat_map(|f| t.lines(f, 1000.min(end - f), &DARK).rows).collect()
}

/// The same from alacritty alone, numbered as leon numbers its screen's top row.
fn plain_rows(term: &Term<VoidListener>, scrolled: usize) -> Vec<Row> {
    let h = term.grid().history_size();
    let all = screen::grid_rows(term, &DARK, -(h as i32), h + term.screen_lines());
    all.into_iter()
        .map(|(l, spans, backgrounds)| Row { line: (scrolled as i64 + l as i64) as usize, spans, backgrounds })
        .collect()
}

/// leon reads as alacritty alone: history size, cursor, and every row's number, text and styles.
fn same(t: &Terminal, reference: &Term<VoidListener>, what: &str) {
    let l = t.lines(0, 0, &DARK);
    assert_eq!(l.history, reference.grid().history_size(), "{what}: history");
    assert_eq!(t.size(), (reference.screen_lines() as u16, reference.columns() as u16), "{what}: size");
    assert_eq!(t.with_term(|t| t.grid().cursor.point), reference.grid().cursor.point, "{what}: cursor");
    let (got, want) = (rows(t), plain_rows(reference, l.scrolled));
    assert_eq!(got.len(), want.len(), "{what}: rows");
    for (g, w) in got.iter().zip(&want) {
        let json = |r: &Row| serde_json::to_string(r).unwrap();
        assert_eq!(json(g), json(w), "{what}: line {} ({:?})", w.line, screen::row_text(&w.spans));
    }
}

#[test]
fn a_pane_grown_after_its_history_settled_shows_what_alacritty_alone_shows() {
    let bytes = output(3000, 0);
    let (t, mut reference) = (leon(&bytes, 24), plain(&bytes, 24));
    assert_eq!(held(&t), 0, "the history is in the store");
    same(&t, &reference, "before");
    t.resize(40, 80).unwrap();
    reference.resize(Size(40, 80));
    same(&t, &reference, "grown to 40 rows");
    let top = t.lines(t.scrolled(), 1, &DARK).rows;
    assert_eq!(screen::row_text(&top[0].spans), "link 2961 after", "the new top row is history's");
}

#[test]
fn a_grow_with_a_width_change_pulls_rows_down_too() {
    // ten 100-character lines wrap on the screen at 80 and 60 and join at 120
    let bytes = output(3000, 10);
    for (rows, cols) in [(40, 100), (40, 60), (24, 120), (60, 120)] {
        let (t, mut reference) = (leon(&bytes, 24), plain(&bytes, 24));
        assert_eq!(held(&t), 0, "the history is in the store");
        t.resize(rows, cols).unwrap();
        reference.resize(Size(rows.into(), cols.into()));
        same(&t, &reference, &format!("24x80 to {rows}x{cols}"));
    }
}

#[test]
fn shrink_then_grow_round_trips_through_the_store() {
    // 901 rows of history, under alacritty's window: they stay alacritty's
    let bytes = output(1300, 0);
    let (t, mut reference) = (leon(&bytes, 400), plain(&bytes, 400));
    assert_eq!((held(&t), t.lines(0, 0, &DARK).history), (901, 901), "alacritty holds the history");
    let before = (rows(&t), t.text(false));
    // 376 more rows go up into the history: past alacritty's window, all of it moves to the store
    t.resize(24, 80).unwrap();
    reference.resize(Size(24, 80));
    same(&t, &reference, "shrunk");
    assert_eq!(held(&t), 0, "the history is in the store");
    t.resize(400, 80).unwrap();
    reference.resize(Size(400, 80));
    same(&t, &reference, "grown back");
    let after = (rows(&t), t.text(false));
    let text = |rows: &[Row]| {
        rows.iter().map(|r| serde_json::to_string(&(&r.spans, &r.backgrounds)).unwrap()).collect::<Vec<_>>()
    };
    assert_eq!(text(&after.0), text(&before.0), "every row as it was, in order");
    assert_eq!(after.1, before.1, "capture as it was");
}

#[test]
fn capture_and_find_agree_with_alacritty_alone_after_a_grow() {
    let bytes = output(3000, 10);
    let (t, mut reference) = (leon(&bytes, 24), plain(&bytes, 24));
    t.resize(40, 120).unwrap();
    reference.resize(Size(40, 120));
    let whole = |term: &Term<VoidListener>| {
        use alacritty_terminal::index::{Column, Point};
        let start = Point::new(term.topmost_line(), Column(0));
        term.bounds_to_string(start, Point::new(term.bottommost_line(), term.last_column()))
    };
    assert_eq!(t.text(false), whole(&reference), "capture");
    assert!(held(&t) > 0, "rows came back into alacritty's history");
    let boundary = t.scrolled() - held(&t);
    for (query, regex) in [("needle", false), ("plain 29\\d\\d bold", true), ("long-00\\d", true), ("中文29", false)]
    {
        let q = Query { query: query.into(), regex, case_sensitive: true, limit: 10_000 };
        let (got, want) = (t.find(&q).unwrap(), search::find(&reference, t.scrolled(), &q).unwrap());
        assert!(!want.matches.is_empty(), "{query} finds something");
        assert_eq!(got.matches, want.matches, "{query}");
        let sides =
            (got.matches.iter().any(|m| m.end.line < boundary), got.matches.iter().any(|m| m.start.line >= boundary));
        if query == "needle" {
            assert_eq!(sides, (true, true), "matches on both sides of the store's end");
        }
    }
}

/// What the screen shows and where the cursor is, as alacritty alone shows it, and capture alike.
/// For a width change: alacritty alone re-wraps its whole history, the store keeps its rows.
fn same_screen(t: &Terminal, reference: &Term<VoidListener>, what: &str) {
    let lines = reference.screen_lines();
    assert_eq!(t.size(), (lines as u16, reference.columns() as u16), "{what}: size");
    assert_eq!(t.with_term(|t| t.grid().cursor.point), reference.grid().cursor.point, "{what}: cursor");
    let got = t.lines(t.scrolled(), lines, &DARK).rows;
    let want = plain_rows(reference, t.scrolled()).split_off(reference.grid().history_size());
    let json = |r: &[Row]| r.iter().map(|r| serde_json::to_string(r).unwrap()).collect::<Vec<_>>();
    assert_eq!(json(&got), json(&want), "{what}: screen");
    let whole = |term: &Term<VoidListener>| {
        use alacritty_terminal::index::{Column, Point};
        let start = Point::new(term.topmost_line(), Column(0));
        term.bounds_to_string(start, Point::new(term.bottommost_line(), term.last_column()))
    };
    assert_eq!(t.text(true), whole(reference).trim_end(), "{what}: every line whole and in order");
}

fn sized(bytes: &[u8], rows: u16, cols: u16) -> (Terminal, Term<VoidListener>) {
    let t = Terminal::spawn_sized(spec(), bytes, Some((rows, cols)), 10_000, |_| {}).unwrap();
    assert_eq!(held(&t), 0, "the history is in the store");
    let mut term = Term::new(
        Config { scrolling_history: 10_000, ..Config::default() },
        &Size(rows.into(), cols.into()),
        VoidListener,
    );
    Processor::<alacritty_terminal::vte::ansi::StdSyncHandler>::new().advance(&mut term, bytes);
    (t, term)
}

#[test]
fn widening_three_times_fills_the_screen_from_lines_three_rows_deep() {
    // 119 characters: three rows at 40 columns, one at 120
    let bytes: String = (0..3000).map(|n| format!("line-{n:04}-{}\r\n", "x".repeat(109))).collect();
    for (rows, cols) in [(24, 120), (40, 120)] {
        let (t, mut reference) = sized(bytes.as_bytes(), 24, 40);
        t.resize(rows, cols).unwrap();
        reference.resize(Size(rows.into(), cols.into()));
        same_screen(&t, &reference, &format!("24x40 to {rows}x{cols}"));
    }
}

#[test]
fn a_pane_narrowed_then_grown_fills_its_new_rows() {
    // 100 characters, kept whole by the store at 120 columns; every third has a wide character
    // that doesn't fit at 80 and moves on with a leading spacer
    let line = |n: usize| match n % 3 {
        0 => format!("line-{n:04}-{}中{}", "y".repeat(69), "w".repeat(19)),
        1 => format!("\x1b[32mline-{n:04}-\x1b[0m{}", "g".repeat(90)),
        _ => format!("line-{n:04}-{}", "v".repeat(90)),
    };
    let bytes: String = (0..3000).map(|n| line(n) + "\r\n").collect();
    let (t, mut reference) = sized(bytes.as_bytes(), 24, 120);
    t.resize(24, 80).unwrap();
    reference.resize(Size(24, 80));
    same_screen(&t, &reference, "narrowed to 24x80");
    t.resize(80, 80).unwrap();
    reference.resize(Size(80, 80));
    same_screen(&t, &reference, "grown to 80x80");
}

#[test]
fn a_resize_alacritty_can_fill_alone_moves_nothing() {
    // a shrink leaves 376 rows in alacritty, under its window, with older ones in the store
    let bytes = output(3000, 0);
    let (t, mut reference) = (leon(&bytes, 400), plain(&bytes, 400));
    t.resize(24, 80).unwrap();
    reference.resize(Size(24, 80));
    assert_eq!(held(&t), 376, "alacritty holds what the shrink pushed");
    // a grow and a widening it can fill from its own rows: none of them goes to the store
    t.resize(100, 80).unwrap();
    reference.resize(Size(100, 80));
    assert_eq!(held(&t), 300, "the grow took its 76 rows from alacritty's");
    t.resize(100, 120).unwrap();
    reference.resize(Size(100, 120));
    assert_eq!(held(&t), 300, "the widening joined nothing and moved nothing");
    same(&t, &reference, "after both");
}
