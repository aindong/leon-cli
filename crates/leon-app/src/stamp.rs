//! Text written over a frame's cells: copy mode's badge and search prompt, and hint labels. What
//! was under it is cut out of its spans, so the frame still has one span per cell.

use crate::render::{BgRect, Frame, Span};

/// Write `text` over row `row` from column `col` (cut at the frame's edge), `fg` on `bg`, bold. A
/// wide character or marked text it covers part of goes whole.
pub fn stamp(f: &mut Frame, row: usize, col: usize, text: &str, fg: u32, bg: u32) {
    let end = (col + text.chars().count()).min(f.cols);
    let Some(spans) = f.rows.get_mut(row).filter(|_| col < end) else { return };
    let mut out = Vec::new();
    for s in spans.drain(..) {
        let s_end = s.col + s.cells;
        if s_end <= col || s.col >= end {
            out.push(s);
            continue;
        }
        if s.cells != s.text.chars().count() {
            continue;
        }
        let chars: Vec<char> = s.text.chars().collect();
        if s.col < col {
            out.push(Span { text: chars[..col - s.col].iter().collect(), cells: col - s.col, ..s.clone() });
        }
        if s_end > end {
            out.push(Span { col: end, text: chars[end - s.col..].iter().collect(), cells: s_end - end, ..s.clone() });
        }
    }
    let text: String = text.chars().take(end - col).collect();
    let label = Span { col, cells: end - col, text, fg, bold: true, italic: false, underline: false, strikeout: false };
    out.push(label);
    out.sort_by_key(|s| s.col);
    *spans = out;
    f.backgrounds.push(BgRect { line: row, col, len: end - col, color: bg });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(col: usize, text: &str) -> Span {
        let cells = text.chars().count();
        Span { col, cells, text: text.into(), fg: 1, bold: false, italic: false, underline: false, strikeout: false }
    }

    #[test]
    fn a_label_replaces_the_cells_under_it() {
        let mut f = Frame { lines: 1, cols: 12, rows: vec![vec![span(0, "hello world")]], ..Frame::default() };
        stamp(&mut f, 0, 3, "ab", 9, 7);
        assert_eq!(f.text().trim_end(), "helab world");
        assert_eq!(f.rows[0].iter().map(|s| (s.col, s.cells)).collect::<Vec<_>>(), [(0, 3), (3, 2), (5, 6)]);
        assert_eq!(f.backgrounds.last(), Some(&BgRect { line: 0, col: 3, len: 2, color: 7 }));
        stamp(&mut f, 0, 10, "xyz", 9, 7);
        assert_eq!(f.text().trim_end(), "helab worlxy", "cut at the edge");
        stamp(&mut f, 5, 0, "x", 9, 7); // no such row: nothing
    }
}
