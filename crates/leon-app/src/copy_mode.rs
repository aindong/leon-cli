//! Copy mode's model (tmux's copy mode, vi keys): a cursor over a pane's scrollback and screen,
//! a selection anchored where `v` or `V` was pressed, and search steps over find's matches. No
//! GPUI, so any client can drive it; lines are absolute (`View::scrolled`), as selections and find
//! are, so the cursor stays on its text as output arrives.

use leon_core::search::{Kind, Pos, Range};
use leon_lua::keys::{Chord, Key};

/// A word motion reads at most this many lines looking for the next word.
const MAX_LINES: usize = 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motion {
    Left,
    Down,
    Up,
    Right,
    /// `w`: the next word's start; a run of punctuation is a word too.
    WordNext,
    /// `b`: the start of this word, or of the one before.
    WordBack,
    LineStart,
    /// `$`: the line's last non-blank cell.
    LineEnd,
    /// `g` and `G`: the oldest line kept and the screen's last.
    Top,
    Bottom,
    /// ctrl+u and ctrl+d: half the pane's rows.
    HalfUp,
    HalfDown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Move(Motion),
    /// `v` (characters) or `V` (lines): start a selection at the cursor, switch its kind, or end it.
    Select(Kind),
    /// `/` (down) or `?` (up): ask for text to search for.
    Search {
        forward: bool,
    },
    /// `n`, or `N` (`reverse`): the next match in the last search's direction.
    Next {
        reverse: bool,
    },
    /// `y` or Enter: copy the selection and leave.
    Yank,
    /// Esc, `q` or ctrl+c: leave.
    Exit,
}

/// What a key does in copy mode; None for one it doesn't know (⌘ and ⌥ chords included).
pub fn op(c: &Chord) -> Option<Op> {
    use Motion::*;
    let mut m = c.mods;
    if m.cmd || m.alt {
        return None;
    }
    let key = match c.key {
        Key::Char(ch) => match leon_core::keys::unshifted(ch) {
            Some(base) => {
                m.shift = true; // `$` as typed is shift+4
                Key::Char(base)
            }
            None => Key::Char(ch),
        },
        ref k => k.clone(),
    };
    Some(match (m.ctrl, m.shift, key) {
        (false, false, Key::Char('h') | Key::Named("left")) => Op::Move(Left),
        (false, false, Key::Char('j') | Key::Named("down")) => Op::Move(Down),
        (false, false, Key::Char('k') | Key::Named("up")) => Op::Move(Up),
        (false, false, Key::Char('l') | Key::Named("right")) => Op::Move(Right),
        (false, false, Key::Char('w')) => Op::Move(WordNext),
        (false, false, Key::Char('b')) => Op::Move(WordBack),
        (false, false, Key::Char('0') | Key::Named("home")) => Op::Move(LineStart),
        (false, true, Key::Char('4')) | (false, false, Key::Named("end")) => Op::Move(LineEnd),
        (false, false, Key::Char('g')) => Op::Move(Top),
        (false, true, Key::Char('g')) => Op::Move(Bottom),
        (true, false, Key::Char('u')) => Op::Move(HalfUp),
        (true, false, Key::Char('d')) => Op::Move(HalfDown),
        (false, false, Key::Char('v')) => Op::Select(Kind::Simple),
        (false, true, Key::Char('v')) => Op::Select(Kind::Line),
        (false, false, Key::Char('/')) => Op::Search { forward: true },
        (false, true, Key::Char('/')) => Op::Search { forward: false },
        (false, false, Key::Char('n')) => Op::Next { reverse: false },
        (false, true, Key::Char('n')) => Op::Next { reverse: true },
        (false, false, Key::Char('y') | Key::Named("enter")) => Op::Yank,
        (false, false, Key::Char('q') | Key::Named("escape")) | (true, false, Key::Char('c')) => Op::Exit,
        _ => return None,
    })
}

/// The lines the cursor moves over: `oldest..=last`, `cols` wide, `rows` of them on screen.
#[derive(Debug, Clone, Copy)]
pub struct Area {
    pub oldest: usize,
    pub last: usize,
    pub cols: usize,
    pub rows: usize,
}

/// Where `m` takes the cursor from `at`; `row(line)` gives a line's cells, one char per cell (a
/// wide char's second cell `\0`). The cursor never rests on a second cell.
pub fn motion(at: Pos, m: Motion, area: &Area, row: &mut dyn FnMut(usize) -> Vec<char>) -> Pos {
    let line = |l: usize| l.clamp(area.oldest, area.last);
    let half = (area.rows / 2).max(1);
    let last = area.cols.saturating_sub(1);
    match m {
        Motion::Left => {
            let cells = row(at.line);
            let mut col = at.col.saturating_sub(1);
            while col > 0 && cell(&cells, col) == '\0' {
                col -= 1;
            }
            Pos { col, ..at }
        }
        Motion::Right => {
            let cells = row(at.line);
            let next = (at.col + 1..=last).find(|&c| cell(&cells, c) != '\0');
            Pos { col: next.unwrap_or(at.col), ..at }
        }
        Motion::Up => Pos { line: line(at.line.saturating_sub(1)), ..at },
        Motion::Down => Pos { line: line(at.line + 1), ..at },
        Motion::HalfUp => Pos { line: line(at.line.saturating_sub(half)), ..at },
        Motion::HalfDown => Pos { line: line(at.line + half), ..at },
        Motion::LineStart => Pos { col: 0, ..at },
        Motion::LineEnd => {
            let cells = row(at.line);
            let end = (0..area.cols).rev().find(|&c| !matches!(cell(&cells, c), '\0') && class(cell(&cells, c)) != 0);
            Pos { col: end.unwrap_or(0), ..at }
        }
        Motion::Top => Pos { line: area.oldest, col: 0 },
        Motion::Bottom => Pos { line: area.last, col: 0 },
        Motion::WordNext => word_next(at, area, &mut |l| whole(row(l))),
        Motion::WordBack => word_back(at, area, &mut |l| whole(row(l))),
    }
}

/// A row whose second cells read as the char they belong to, so word runs go over them.
fn whole(mut cells: Vec<char>) -> Vec<char> {
    for i in 1..cells.len() {
        if cells[i] == '\0' {
            cells[i] = cells[i - 1];
        }
    }
    cells
}

fn cell(cells: &[char], col: usize) -> char {
    cells.get(col).copied().unwrap_or(' ')
}

/// 0 blank, 1 word, 2 punctuation.
fn class(c: char) -> u8 {
    match c {
        c if c.is_whitespace() => 0,
        c if c.is_alphanumeric() || c == '_' => 1,
        _ => 2,
    }
}

/// Past the run under the cursor, then past blanks (a line's end is one), to the next word; with
/// none, the end of the run.
fn word_next(at: Pos, area: &Area, row: &mut dyn FnMut(usize) -> Vec<char>) -> Pos {
    let (mut line, mut col, mut cells) = (at.line, at.col, row(at.line));
    let start = class(cell(&cells, col));
    let mut end = at;
    while start != 0 && col < area.cols && class(cell(&cells, col)) == start {
        end = Pos { line, col };
        col += 1;
    }
    for _ in 0..MAX_LINES {
        if let Some(c) = (col..area.cols).find(|&c| class(cell(&cells, c)) != 0) {
            return Pos { line, col: c };
        }
        if line >= area.last {
            break;
        }
        (line, col, cells) = (line + 1, 0, row(line + 1));
    }
    end
}

/// Back past blanks (across lines) to a word, then to that word's start.
fn word_back(at: Pos, area: &Area, row: &mut dyn FnMut(usize) -> Vec<char>) -> Pos {
    let (mut line, mut col, mut cells) = (at.line, at.col, row(at.line));
    for _ in 0..MAX_LINES {
        if let Some(c) = (0..col).rev().find(|&c| class(cell(&cells, c)) != 0) {
            let k = class(cell(&cells, c));
            let start = (0..=c).rev().take_while(|&i| class(cell(&cells, i)) == k).last().unwrap_or(c);
            return Pos { line, col: start };
        }
        if line <= area.oldest {
            break;
        }
        (line, col, cells) = (line - 1, area.cols, row(line - 1));
    }
    at
}

/// The match to go to from `at`: the first that starts after it (`forward`) or the last that
/// starts before it, wrapping round; None with no matches. `matches` are oldest first.
pub fn pick(matches: &[Range], at: Pos, forward: bool) -> Option<Pos> {
    let mut starts = matches.iter().map(|m| m.start);
    match forward {
        true => starts.clone().find(|s| *s > at).or_else(|| starts.next()),
        false => starts.clone().rfind(|s| *s < at).or_else(|| starts.next_back()),
    }
}

/// Copy mode on one pane.
#[derive(Debug, Clone, PartialEq)]
pub struct CopyMode {
    pub block: String,
    pub cursor: Pos,
    /// The selection's kind and anchor; the cursor is its other end.
    pub selection: Option<(Kind, Pos)>,
    /// A search being typed: its direction (true: down) and text.
    pub prompt: Option<(bool, String)>,
    /// The last search, the same way.
    pub search: Option<(bool, String)>,
    /// How many matches the last search found, when find stopped short of them all.
    pub truncated: Option<usize>,
}

impl CopyMode {
    pub fn new(block: String, cursor: Pos) -> CopyMode {
        CopyMode { block, cursor, selection: None, prompt: None, search: None, truncated: None }
    }

    /// `v` or `V`: a selection of `kind` from the cursor; the same key again ends it, the other
    /// one switches kind and keeps the anchor.
    pub fn select(&mut self, kind: Kind) {
        self.selection = match self.selection {
            Some((k, _)) if k == kind => None,
            Some((_, anchor)) => Some((kind, anchor)),
            None => Some((kind, self.cursor)),
        };
    }
}
