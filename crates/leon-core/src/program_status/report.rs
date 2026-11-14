//! The report grammar (docs/spec.md §5): `pairs := pair (":" pair)*`, `pair := key "=" value`.

use base64::engine::{general_purpose::GeneralPurpose, DecodePaddingMode, GeneralPurposeConfig};
use base64::{alphabet, Engine as _};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    Idle,
    Working,
    Done,
    Blocked,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Permission,
    Question,
    Auth,
}

/// One program's status. `title` and `msg` are decoded, single-line and free of controls and
/// invisible characters; a renderer must still bidi-isolate them and clip their height.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Record {
    pub state: State,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<Kind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub msg: Option<String>,
}

/// A parsed OSC 7501 payload. `id: None` is the root record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    Query,
    Set { id: Option<String>, record: Record },
    Clear { id: Option<String> },
}

/// Why a whole report was ignored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ignored {
    NoState,
    BadState,
    BadId,
    /// A size limit was exceeded; names the key (`key` for the key itself).
    TooLong(&'static str),
}

const B64: GeneralPurpose = GeneralPurpose::new(
    &alphabet::STANDARD,
    GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::Indifferent),
);

/// The last well-formed value of each known key.
#[derive(Default)]
struct Pairs<'a> {
    state: Option<&'a str>,
    id: Option<&'a str>,
    kind: Option<&'a str>,
    progress: Option<&'a str>,
    app: Option<&'a str>,
    title: Option<&'a str>,
    msg: Option<&'a str>,
}

/// Parses the bytes between `OSC 7501 ;` and ST.
pub fn parse(payload: &[u8]) -> Result<Message, Ignored> {
    let payload = payload.trim_ascii();
    if payload == b"?" {
        return Ok(Message::Query);
    }
    let mut pairs = Pairs::default();
    for pair in payload.split(|&b| b == b':') {
        let Some(eq) = pair.iter().position(|&b| b == b'=') else { continue };
        let (key, value) = (pair[..eq].trim_ascii(), pair[eq + 1..].trim_ascii());
        if key.is_empty() || !key.iter().all(u8::is_ascii_lowercase) {
            continue;
        }
        if key.len() > 16 {
            return Err(Ignored::TooLong("key"));
        }
        // Skipping a malformed id would land the report on the root, so every id must be valid.
        if key == b"id" && !valid_id(value) {
            return Err(Ignored::BadId);
        }
        if !value.iter().all(|&b| value_byte(b)) {
            continue;
        }
        let value = std::str::from_utf8(value).expect("value bytes are ASCII");
        // Limits hold for every occurrence, even one a later duplicate replaces. (encoded, decoded)
        let (slot, max, max_decoded, name) = match key {
            b"state" => (&mut pairs.state, usize::MAX, usize::MAX, "state"),
            b"id" => (&mut pairs.id, usize::MAX, usize::MAX, "id"),
            b"kind" => (&mut pairs.kind, usize::MAX, usize::MAX, "kind"),
            b"progress" => (&mut pairs.progress, usize::MAX, usize::MAX, "progress"),
            b"app" => (&mut pairs.app, 32, usize::MAX, "app"),
            b"title" => (&mut pairs.title, 256, 192, "title"),
            b"msg" => (&mut pairs.msg, 2732, 2048, "msg"),
            _ => continue,
        };
        if value.len() > max || (max_decoded < usize::MAX && B64.decode(value).map_or(0, |b| b.len()) > max_decoded) {
            return Err(Ignored::TooLong(name));
        }
        *slot = Some(value);
    }

    let state = match pairs.state {
        None => return Err(Ignored::NoState),
        Some("idle") => State::Idle,
        Some("working") => State::Working,
        Some("done") => State::Done,
        Some("blocked") => State::Blocked,
        Some("error") => State::Error,
        Some("clear") => return Ok(Message::Clear { id: pairs.id.map(String::from) }),
        Some(_) => return Err(Ignored::BadState),
    };
    let id = pairs.id.map(String::from);
    let kind = match (state, pairs.kind) {
        (State::Blocked, Some("permission")) => Some(Kind::Permission),
        (State::Blocked, Some("question")) => Some(Kind::Question),
        (State::Blocked, Some("auth")) => Some(Kind::Auth),
        _ => None,
    };
    let progress = pairs
        .progress
        .filter(|p| matches!(state, State::Working | State::Blocked) && (1..=3).contains(&p.len()))
        .filter(|p| p.bytes().all(|b| b.is_ascii_digit()))
        .and_then(|p| p.parse().ok())
        .filter(|&p| p <= 100);
    let app = pairs.app.filter(|a| !a.is_empty() && a.bytes().all(segment_byte)).map(String::from);
    let (title, msg) = (text(pairs.title), text(pairs.msg));
    Ok(Message::Set { id, record: Record { state, kind, progress, app, title, msg } })
}

fn value_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b"_.,+/=-".contains(&b)
}

fn segment_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b"_.+-".contains(&b)
}

/// `segment ("/" segment)*`, segments of 1–32 bytes, at most 8 deep and 128 bytes in all.
fn valid_id(id: &[u8]) -> bool {
    let segment_ok = |s: &[u8]| (1..=32).contains(&s.len()) && s.iter().all(|&b| segment_byte(b));
    id.len() <= 128 && id.split(|&b| b == b'/').count() <= 8 && id.split(|&b| b == b'/').all(segment_ok)
}

/// Base64 UTF-8 text; anything undecodable or unsafe drops just the field.
fn text(value: Option<&str>) -> Option<String> {
    let bytes = B64.decode(value?).ok()?;
    String::from_utf8(bytes).ok().filter(|s| safe_text(s))
}

/// One line with nothing hidden: no controls (C0, DEL, C1), line breaks, bidi controls or invisible
/// characters, and something visible. ZWNJ, ZWJ and the emoji presentation selectors stay, but only
/// right after a visible character (ZWJ also after a selector, as in 🏳️‍🌈), so they can't form hidden
/// runs. Other variation selectors and tag characters stay out: they can smuggle a byte per character.
fn safe_text(s: &str) -> bool {
    let mut prev = None;
    for c in s.chars() {
        if !allowed(c, prev) {
            return false;
        }
        prev = Some(c);
    }
    s.chars().any(visible)
}

/// `s` without what `safe_text` refuses, for program text we keep rather than drop (titles, pwds).
pub(crate) fn strip_hidden(s: &str) -> String {
    let mut prev = None;
    s.chars()
        .filter(|&c| {
            let ok = allowed(c, prev);
            if ok {
                prev = Some(c);
            }
            ok
        })
        .collect()
}

/// Whether `c` may follow `prev` in shown text.
fn allowed(c: char, prev: Option<char>) -> bool {
    match c {
        '\u{200d}' => prev.is_some_and(|p| visible(p) || matches!(p, '\u{fe0e}' | '\u{fe0f}')),
        '\u{200c}' | '\u{fe0e}' | '\u{fe0f}' => prev.is_some_and(visible),
        _ => !c.is_control() && !invisible(c),
    }
}

/// Not whitespace and not one of the joiners `safe_text` allows.
fn visible(c: char) -> bool {
    !c.is_whitespace() && !matches!(c, '\u{200c}' | '\u{200d}' | '\u{fe0e}' | '\u{fe0f}')
}

/// Unicode's Default_Ignorable_Code_Point set, plus the line/paragraph separators and interlinear
/// annotation characters.
fn invisible(c: char) -> bool {
    matches!(
        c,
        '\u{ad}'
            | '\u{34f}'
            | '\u{61c}'
            | '\u{115f}'..='\u{1160}'
            | '\u{17b4}'..='\u{17b5}'
            | '\u{180b}'..='\u{180f}'
            | '\u{200b}'..='\u{200f}'
            | '\u{2028}'..='\u{202e}'
            | '\u{2060}'..='\u{206f}'
            | '\u{3164}'
            | '\u{fe00}'..='\u{fe0f}'
            | '\u{feff}'
            | '\u{ffa0}'
            | '\u{fff0}'..='\u{fffb}'
            | '\u{1bca0}'..='\u{1bca3}'
            | '\u{1d173}'..='\u{1d17a}'
            | '\u{e0000}'..='\u{e0fff}'
    )
}
