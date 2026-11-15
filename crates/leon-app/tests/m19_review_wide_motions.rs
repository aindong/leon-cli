//! Review (M19): copy mode's motions never stop on a wide character's second cell.
use leon_app::copy_mode::{motion, Area, Motion};
use leon_core::search::Pos;

fn row(_: usize) -> Vec<char> {
    let mut c: Vec<char> = "🙂\0 foo 漢\0".chars().collect();
    c.resize(10, ' ');
    c
}

fn go(col: usize, m: Motion) -> usize {
    let area = Area { oldest: 0, last: 0, cols: 10, rows: 1 };
    motion(Pos { line: 0, col }, m, &area, &mut row).col
}

#[test]
fn motions_skip_a_wide_chars_second_cell() {
    assert_eq!(go(0, Motion::WordNext), 3, "w from an emoji goes to foo");
    assert_eq!(go(3, Motion::WordBack), 0, "b from foo goes to the emoji's first cell");
    assert_eq!(go(0, Motion::Right), 2, "l steps over the second cell");
    assert_eq!(go(2, Motion::Left), 0, "h too");
    assert_eq!(go(0, Motion::LineEnd), 7, "$ lands on the last wide char's first cell");
}
