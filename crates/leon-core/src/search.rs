//! Text in a terminal's grid, in absolute line numbers (`View::scrolled`): find, selections and
//! the link under a point. alacritty follows wrapped lines and wide characters for all three; this
//! maps its points to numbers that stay put as output arrives.

use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Boundary, Column, Line, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::search::RegexSearch;
use alacritty_terminal::term::Term;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A cell: its absolute line and its column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Pos {
    pub line: usize,
    pub col: usize,
}

/// From `start` to `end`, both cells included, in reading order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Range {
    pub start: Pos,
    pub end: Pos,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Query {
    pub query: String,
    /// The query is a regular expression (Rust `regex` syntax); else it is plain text.
    #[serde(default)]
    pub regex: bool,
    #[serde(default)]
    pub case_sensitive: bool,
    /// At most this many matches (and at most `MAX_MATCHES`).
    #[serde(default = "max_matches")]
    pub limit: usize,
}

pub const MAX_MATCHES: usize = 10_000;
/// Longer links are refused: a program could otherwise hand the app any amount of text to show.
pub const MAX_URI: usize = 2048;

fn max_matches() -> usize {
    MAX_MATCHES
}

#[derive(Debug, Default, Serialize, JsonSchema)]
pub struct Found {
    /// Oldest first.
    pub matches: Vec<Range>,
    /// More matches exist than were returned.
    pub truncated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// From one cell to the other, in reading order.
    Simple,
    /// Whole words (a double-click).
    Word,
    /// Whole lines, a wrapped line as one (a triple-click).
    Line,
    /// The rectangle with the two cells at its corners.
    Block,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Selected {
    pub start: Pos,
    pub end: Pos,
    /// The text it copies, when asked for: no newline inside a wrapped line.
    pub text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Link {
    pub uri: String,
    pub start: Pos,
    pub end: Pos,
}

/// Whether opening `uri` should show it first: anything but a web link could start an app or
/// open a file.
pub fn needs_confirm(uri: &str) -> bool {
    let scheme = uri.split_once(':').map(|(s, _)| s.to_ascii_lowercase());
    !matches!(scheme.as_deref(), Some("http" | "https"))
}

/// alacritty's point for `pos` (line 0 is the screen's top), if that cell is in the grid.
fn point<T>(term: &Term<T>, scrolled: usize, pos: Pos) -> Option<Point> {
    let line = i32::try_from(pos.line as i64 - scrolled as i64).ok()?;
    let inside = (term.topmost_line().0..=term.bottommost_line().0).contains(&line) && pos.col < term.columns();
    inside.then_some(Point::new(Line(line), Column(pos.col)))
}

fn pos(scrolled: usize, p: Point) -> Pos {
    Pos { line: (scrolled as i64 + p.line.0 as i64) as usize, col: p.column.0 }
}

pub fn find<T>(term: &Term<T>, scrolled: usize, q: &Query) -> Result<Found, String> {
    let mut found = Found::default();
    if q.query.is_empty() {
        return Ok(found);
    }
    let text = if q.regex { q.query.clone() } else { regex_syntax::escape(&q.query) };
    // alacritty ignores case unless the query has a capital; the flag decides instead
    let flag = if q.case_sensitive { "(?-i)" } else { "(?i)" };
    let mut regex = RegexSearch::new(&format!("{flag}{text}")).map_err(|e| e.to_string())?;
    let limit = q.limit.min(MAX_MATCHES);
    let end = Point::new(term.bottommost_line(), term.last_column());
    let mut from = Point::new(term.topmost_line(), Column(0));
    while let Some(m) = term.regex_search_right(&mut regex, from, end) {
        if *m.start() < from {
            break; // it wrapped around to the top
        }
        if found.matches.len() == limit {
            found.truncated = true;
            break;
        }
        found.matches.push(Range { start: pos(scrolled, *m.start()), end: pos(scrolled, *m.end()) });
        // past the match, and at least a cell on, so a pattern that matches nothing still ends
        let next = (*m.end()).max(from).add(term, Boundary::Grid, 1);
        if *m.end() >= end || next == from {
            break;
        }
        from = next;
    }
    Ok(found)
}

/// The selection of `kind` from `anchor` to `head` (`head` is kept inside the grid). None when
/// `anchor` isn't in the grid. Needs the terminal mutably only to borrow alacritty's own
/// selection-to-text, and puts its selection back.
pub fn select<T>(
    term: &mut Term<T>,
    scrolled: usize,
    kind: Kind,
    anchor: Pos,
    head: Pos,
    text: bool,
) -> Option<Selected> {
    let a = point(term, scrolled, anchor)?;
    let last = Pos { line: scrolled + term.screen_lines() - 1, col: term.columns() - 1 };
    let oldest = scrolled.saturating_sub(term.grid().history_size());
    let head = Pos { line: head.line.clamp(oldest, last.line), col: head.col.min(last.col) };
    let b = point(term, scrolled, head)?;
    let (mut lo, mut hi) = if kind == Kind::Block {
        let (l, r) = (a.column.min(b.column), a.column.max(b.column));
        (Point::new(a.line.min(b.line), l), Point::new(a.line.max(b.line), r))
    } else {
        (a.min(b), a.max(b))
    };
    // half a wide character takes the whole one
    if term.grid()[lo].flags.contains(Flags::WIDE_CHAR_SPACER) && lo.column.0 > 0 {
        lo.column -= 1;
    }
    if term.grid()[hi].flags.contains(Flags::WIDE_CHAR) && hi.column < term.last_column() {
        hi.column += 1;
    }
    let ty = match kind {
        Kind::Simple => SelectionType::Simple,
        Kind::Word => SelectionType::Semantic,
        Kind::Line => SelectionType::Lines,
        Kind::Block => SelectionType::Block,
    };
    let mut sel = Selection::new(ty, lo, Side::Left);
    sel.update(hi, Side::Right);
    let range = sel.to_range(term)?;
    let text = text.then(|| {
        let kept = term.selection.replace(sel);
        let t = term.selection_to_string();
        term.selection = kept;
        t.unwrap_or_default()
    });
    Some(Selected { start: pos(scrolled, range.start), end: pos(scrolled, range.end), text })
}

/// Detected links: a scheme and `://`, or `mailto:`, then anything up to a space or a character
/// URLs leave out.
const URL: &str = r#"(?i)(?:[a-z][a-z0-9+.-]{1,31}://|mailto:)[^\s<>"'`{}|\\^\x00-\x1f\x7f-\x9f]+"#;

/// The link on the cell at `at`: an OSC 8 hyperlink, else a URL found in its logical line.
pub fn link_at<T>(term: &Term<T>, scrolled: usize, at: Pos) -> Option<Link> {
    let p = point(term, scrolled, at)?;
    let (uri, start, end) = match term.grid()[p].hyperlink() {
        Some(h) => {
            let same = |q: Point| term.grid()[q].hyperlink().as_ref() == Some(&h);
            let (mut start, mut end) = (p, p);
            // bounded: a link is at most a few screens of cells
            for _ in 0..MAX_URI * 4 {
                let before = start.sub(term, Boundary::Grid, 1);
                if before == start || !same(before) {
                    break;
                }
                start = before;
            }
            for _ in 0..MAX_URI * 4 {
                let after = end.add(term, Boundary::Grid, 1);
                if after == end || !same(after) {
                    break;
                }
                end = after;
            }
            (h.uri().to_string(), start, end)
        }
        None => detected(term, p)?,
    };
    let clean = uri.len() <= MAX_URI && crate::program_status::strip_hidden(&uri) == uri;
    (clean && !uri.chars().any(char::is_control)).then(|| Link {
        uri,
        start: pos(scrolled, start),
        end: pos(scrolled, end),
    })
}

fn detected<T>(term: &Term<T>, p: Point) -> Option<(String, Point, Point)> {
    let mut regex = RegexSearch::new(URL).ok()?;
    let (first, last) = (term.line_search_left(p), term.line_search_right(p));
    let mut from = first;
    while let Some(m) = term.regex_search_right(&mut regex, from, last) {
        let (start, mut end) = (*m.start(), *m.end());
        if start < from || start > p {
            return None;
        }
        let mut uri = term.bounds_to_string(start, end);
        // trailing punctuation ends a sentence, not a URL; a ')' only when it closes nothing
        while let Some(c) = uri.chars().last() {
            let unbalanced = c == ')' && uri.matches('(').count() < uri.matches(')').count();
            if !(".,;:!?'\"".contains(c) || unbalanced) {
                break;
            }
            uri.pop();
            end = end.sub(term, Boundary::Grid, 1);
        }
        if p <= end {
            return Some((uri, start, end));
        }
        if *m.end() >= last {
            return None;
        }
        from = m.end().add(term, Boundary::Grid, 1);
    }
    None
}
