//! Hints (tmux-fingers, quick select): the tokens on a screen worth copying (URLs, paths, git
//! hashes, UUIDs, hex numbers, IPv4 addresses, long numbers, and the config's own patterns), each
//! labelled with a letter or two to type.

use std::sync::OnceLock;

use regex::{Regex, RegexBuilder};

/// Label letters, the easiest to reach first (tmux-fingers' alphabet).
const ALPHABET: &str = "asdfqwerzxcvjklmiuopghtybn";
/// The most distinct texts one screen labels: every two-letter label.
pub const MAX_HINTS: usize = 26 * 26;
/// The config's patterns: at most this many, each at most this long.
pub const MAX_PATTERNS: usize = 32;
pub const MAX_PATTERN_LEN: usize = 256;
/// A pattern compiles to at most this much (Rust's regex runs in time linear in the text).
const SIZE_LIMIT: usize = 1 << 20;

/// The built-in rules, URLs first. None has a capture group: the whole match is the text.
const BUILT_IN: &[&str] = &[
    r#"\b(?:https?|ftp|file)://[^\s<>"'`\x00]+"#,
    r"\b[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}\b",
    r"(?:~|\.{1,2})?(?:/[\w.@+-]+)+/?|[\w.@+-]+(?:/[\w.@+-]+)+/?",
    r"\b0x[0-9a-fA-F]+\b",
    r"\b\d{1,3}(?:\.\d{1,3}){3}(?::\d{1,5})?\b",
    r"\b[0-9a-f]{7,40}\b",
    r"\b\d{4,}\b",
];

/// One labelled token: where it starts (row, and column in cells), how many cells it covers.
#[derive(Debug, Clone, PartialEq)]
pub struct Hint {
    pub row: usize,
    pub col: usize,
    pub cells: usize,
    pub text: String,
    pub label: String,
}

/// A hint pattern from the config, checked and compiled.
pub fn compile(pattern: &str) -> Result<Regex, String> {
    if pattern.is_empty() {
        return Err("an empty hint pattern".into());
    }
    if pattern.len() > MAX_PATTERN_LEN {
        return Err(format!("a hint pattern is at most {MAX_PATTERN_LEN} bytes; this one has {}", pattern.len()));
    }
    let built = RegexBuilder::new(pattern).size_limit(SIZE_LIMIT).build();
    built.map_err(|e| format!("bad hint pattern {pattern:?}: {e}"))
}

fn built_in() -> &'static [Regex] {
    static RULES: OnceLock<Vec<Regex>> = OnceLock::new();
    RULES.get_or_init(|| BUILT_IN.iter().map(|p| Regex::new(p).expect("a built-in hint rule compiles")).collect())
}

/// What a built-in match keeps: no trailing punctuation, nor a closing bracket it didn't open.
fn trim(s: &str) -> &str {
    let mut s = s;
    loop {
        let unopened = |open, close| s.ends_with(close) && s.matches(close).count() > s.matches(open).count();
        if s.ends_with(['.', ',', ';', ':', '!', '?', '\'', '"']) || unopened('(', ')') || unopened('[', ']') {
            s = &s[..s.len() - 1];
        } else {
            return s;
        }
    }
}

/// The hints on a screen given as rows of cells (one char per cell, a wide char's second cell
/// `\0`), with `extra` patterns from the config (group 1, when it matched, is the text). Overlaps
/// keep the earliest start, then the longest. A text keeps one label wherever it shows; the
/// shortest labels go to the texts nearest the bottom, and past `MAX_HINTS` texts the top ones
/// are dropped.
pub fn find(rows: &[String], extra: &[Regex]) -> Vec<Hint> {
    let mut found = Vec::new();
    for (r, row) in rows.iter().enumerate() {
        let mut spans: Vec<(usize, usize)> = Vec::new(); // byte ranges
        for (i, re) in built_in().iter().chain(extra).enumerate() {
            for caps in re.captures_iter(row) {
                let Some(m) = caps.get(1).or_else(|| caps.get(0)) else { continue };
                let text = if i < BUILT_IN.len() { trim(m.as_str()) } else { m.as_str() };
                if !text.is_empty() {
                    spans.push((m.start(), m.start() + text.len()));
                }
            }
        }
        spans.sort_by_key(|&(start, end)| (start, std::cmp::Reverse(end)));
        let mut end_of_last = 0;
        for (start, end) in spans {
            if start < end_of_last {
                continue;
            }
            end_of_last = end;
            let raw = &row[start..end];
            let (col, cells) = (row[..start].chars().count(), raw.chars().count());
            found.push(Hint { row: r, col, cells, text: raw.replace('\0', ""), label: String::new() });
        }
    }
    // distinct texts, nearest the bottom first
    let mut texts: Vec<&str> = Vec::new();
    for h in found.iter().rev() {
        if !texts.contains(&h.text.as_str()) {
            texts.push(&h.text);
        }
    }
    texts.truncate(MAX_HINTS);
    let labelled: Vec<(String, String)> = texts.iter().map(|t| t.to_string()).zip(labels(texts.len())).collect();
    found
        .into_iter()
        .filter_map(|mut h| {
            h.label = labelled.iter().find(|(t, _)| *t == h.text)?.1.clone();
            Some(h)
        })
        .collect()
}

/// `n` labels (at most `MAX_HINTS`), shortest first, none the start of another: single letters,
/// and when those don't reach, the last few letters start two-letter labels instead.
pub fn labels(n: usize) -> Vec<String> {
    let a: Vec<char> = ALPHABET.chars().collect();
    let k = a.len();
    let n = n.min(k * k);
    let p = (0..=k).find(|p| (k - p) + p * k >= n).unwrap_or(k);
    let singles = a[..k - p].iter().map(|c| c.to_string());
    let doubles = a[k - p..].iter().flat_map(|x| a.iter().map(move |y| format!("{x}{y}")));
    singles.chain(doubles).take(n).collect()
}
