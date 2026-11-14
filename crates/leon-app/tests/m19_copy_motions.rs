//! M19 criterion 7 (.agent/plans/2026-11-15-m19-tmux-migrants.md): copy mode's keys, motions,
//! selection toggles and search steps, without a window.

use leon_app::copy_mode::{motion, op, pick, Area, CopyMode, Motion, Op};
use leon_core::search::{Kind, Pos, Range};
use leon_lua::keys::parse;

/// Lines 100.. of a 20-column pane showing 4 rows.
const TEXT: [&str; 6] = ["first line", "", "  foo.bar(baz) qux", "", "last  words here", "end"];

fn area() -> Area {
    Area { oldest: 100, last: 105, cols: 20, rows: 4 }
}

fn row(line: usize) -> Vec<char> {
    let mut cells: Vec<char> = TEXT.get(line - 100).copied().unwrap_or("").chars().collect();
    cells.resize(20, ' ');
    cells
}

fn go(at: (usize, usize), m: Motion) -> (usize, usize) {
    let p = motion(Pos { line: at.0, col: at.1 }, m, &area(), &mut row);
    (p.line, p.col)
}

fn key(s: &str) -> Option<Op> {
    op(&parse(s).unwrap().seq.remove(0))
}

#[test]
fn keys_name_the_ops() {
    use Motion::*;
    for (k, m) in [
        ("h", Left),
        ("j", Down),
        ("k", Up),
        ("l", Right),
        ("left", Left),
        ("down", Down),
        ("w", WordNext),
        ("b", WordBack),
        ("0", LineStart),
        ("shift+4", LineEnd),
        ("$", LineEnd),
        ("g", Top),
        ("shift+g", Bottom),
        ("ctrl+u", HalfUp),
        ("ctrl+d", HalfDown),
    ] {
        assert_eq!(key(k), Some(Op::Move(m)), "{k}");
    }
    assert_eq!(key("v"), Some(Op::Select(Kind::Simple)));
    assert_eq!(key("shift+v"), Some(Op::Select(Kind::Line)));
    assert_eq!(key("/"), Some(Op::Search { forward: true }));
    assert_eq!(key("?"), Some(Op::Search { forward: false }));
    assert_eq!(key("n"), Some(Op::Next { reverse: false }));
    assert_eq!(key("shift+n"), Some(Op::Next { reverse: true }));
    for k in ["y", "enter"] {
        assert_eq!(key(k), Some(Op::Yank), "{k}");
    }
    for k in ["escape", "q", "ctrl+c"] {
        assert_eq!(key(k), Some(Op::Exit), "{k}");
    }
    assert_eq!(key("x"), None);
    assert_eq!(key("cmd+c"), None, "⌘ chords go on to the keymap");
}

#[test]
fn hjkl_clamp_at_the_edges() {
    use Motion::*;
    assert_eq!(go((100, 0), Left), (100, 0));
    assert_eq!(go((100, 3), Left), (100, 2));
    assert_eq!(go((100, 19), Right), (100, 19));
    assert_eq!(go((100, 3), Up), (100, 3), "the oldest line");
    assert_eq!(go((105, 3), Down), (105, 3), "the newest line");
    assert_eq!(go((102, 5), Down), (103, 5), "the column stays on a blank line");
}

#[test]
fn words_cross_lines_and_punctuation_runs_are_words() {
    use Motion::*;
    assert_eq!(go((100, 0), WordNext), (100, 6), "first → line");
    assert_eq!(go((100, 6), WordNext), (102, 2), "over the end and a blank line to foo");
    assert_eq!(go((102, 2), WordNext), (102, 5), "foo → .");
    assert_eq!(go((102, 5), WordNext), (102, 6), ". → bar");
    assert_eq!(go((102, 9), WordNext), (102, 10), "( → baz");
    assert_eq!(go((102, 13), WordNext), (102, 15), ") → qux");
    assert_eq!(go((104, 12), WordNext), (105, 0), "here → end");
    assert_eq!(go((105, 0), WordNext), (105, 2), "nothing after: the last word's end");
    assert_eq!(go((102, 15), WordBack), (102, 13), "qux → )");
    assert_eq!(go((102, 6), WordBack), (102, 5), "bar → .");
    assert_eq!(go((102, 2), WordBack), (100, 6), "foo → line, across lines");
    assert_eq!(go((100, 2), WordBack), (100, 0), "inside a word: its start");
    assert_eq!(go((100, 0), WordBack), (100, 0), "nothing before");
}

#[test]
fn line_ends_top_bottom_and_half_pages() {
    use Motion::*;
    assert_eq!(go((102, 7), LineStart), (102, 0));
    assert_eq!(go((102, 0), LineEnd), (102, 17), "the last non-blank");
    assert_eq!(go((101, 4), LineEnd), (101, 0), "a blank line has none");
    assert_eq!(go((103, 4), Top), (100, 0));
    assert_eq!(go((101, 4), Bottom), (105, 0));
    assert_eq!(go((104, 4), HalfUp), (102, 4), "half of 4 rows");
    assert_eq!(go((101, 4), HalfUp), (100, 4));
    assert_eq!(go((101, 4), HalfDown), (103, 4));
    assert_eq!(go((104, 4), HalfDown), (105, 4));
}

#[test]
fn v_and_shift_v_start_switch_and_end_a_selection() {
    let mut c = CopyMode::new("b1".into(), Pos { line: 102, col: 2 });
    assert_eq!(c.selection, None);
    c.select(Kind::Simple);
    assert_eq!(c.selection, Some((Kind::Simple, Pos { line: 102, col: 2 })), "anchored at the cursor");
    c.cursor = Pos { line: 103, col: 0 };
    c.select(Kind::Line);
    assert_eq!(c.selection, Some((Kind::Line, Pos { line: 102, col: 2 })), "V switches, the anchor stays");
    c.select(Kind::Line);
    assert_eq!(c.selection, None, "V again ends it");
}

#[test]
fn search_steps_from_the_cursor_and_wraps() {
    let r = |line, col| Range { start: Pos { line, col }, end: Pos { line, col: col + 2 } };
    let matches = [r(10, 4), r(12, 0), r(12, 8), r(20, 1)];
    let at = |line, col| Pos { line, col };
    assert_eq!(pick(&matches, at(12, 0), true), Some(at(12, 8)), "the next one, not the one under the cursor");
    assert_eq!(pick(&matches, at(12, 0), false), Some(at(10, 4)));
    assert_eq!(pick(&matches, at(20, 1), true), Some(at(10, 4)), "wraps to the first");
    assert_eq!(pick(&matches, at(10, 4), false), Some(at(20, 1)), "wraps to the last");
    assert_eq!(pick(&matches, at(15, 0), true), Some(at(20, 1)));
    assert_eq!(pick(&[], at(15, 0), true), None);
}
