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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Permission,
    Question,
    Auth,
}

/// One program's status. `title` and `msg` are decoded and safe to render.
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
        if key.len() > 16 {
            return Err(Ignored::TooLong("key"));
        }
        if key.is_empty() || !key.iter().all(u8::is_ascii_lowercase) {
            continue;
        }
        if !value.iter().all(|&b| value_byte(b)) {
            // Skipping a malformed id would land the report on the root.
            if key == b"id" {
                return Err(Ignored::BadId);
            }
            continue;
        }
        // Both are ASCII now.
        let (key, value) = (std::str::from_utf8(key).unwrap(), std::str::from_utf8(value).unwrap());
        let (slot, max, name) = match key {
            "state" => (&mut pairs.state, usize::MAX, "state"),
            "id" => (&mut pairs.id, usize::MAX, "id"),
            "kind" => (&mut pairs.kind, usize::MAX, "kind"),
            "progress" => (&mut pairs.progress, usize::MAX, "progress"),
            "app" => (&mut pairs.app, 32, "app"),
            "title" => (&mut pairs.title, 256, "title"),
            "msg" => (&mut pairs.msg, 2732, "msg"),
            _ => continue,
        };
        if value.len() > max {
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
        Some("clear") => return Ok(Message::Clear { id: id(pairs.id)? }),
        Some(_) => return Err(Ignored::BadState),
    };
    let id = id(pairs.id)?;
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
    let title = text(pairs.title, 192, "title")?;
    let msg = text(pairs.msg, 2048, "msg")?;
    Ok(Message::Set { id, record: Record { state, kind, progress, app, title, msg } })
}

fn value_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b"_.,+/=-".contains(&b)
}

fn segment_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b"_.+-".contains(&b)
}

/// Absent is the root; present but invalid is an error, never the root.
fn id(id: Option<&str>) -> Result<Option<String>, Ignored> {
    let Some(id) = id else { return Ok(None) };
    let segment_ok = |s: &str| (1..=32).contains(&s.len()) && s.bytes().all(segment_byte);
    if id.len() > 128 || id.split('/').count() > 8 || !id.split('/').all(segment_ok) {
        return Err(Ignored::BadId);
    }
    Ok(Some(id.to_string()))
}

/// Base64 UTF-8 text. Over `max` decoded bytes discards the report; anything unsafe drops just the field.
fn text(value: Option<&str>, max: usize, key: &'static str) -> Result<Option<String>, Ignored> {
    let Some(Ok(bytes)) = value.map(|v| B64.decode(v)) else { return Ok(None) };
    if bytes.len() > max {
        return Err(Ignored::TooLong(key));
    }
    Ok(String::from_utf8(bytes).ok().filter(|s| !s.is_empty() && s.chars().all(safe_char)))
}

/// No controls (C0, DEL, C1), line breaks, bidi controls or invisible format characters. ZWJ and
/// variation selectors stay: emoji need them.
fn safe_char(c: char) -> bool {
    !c.is_control()
        && !matches!(
            c,
            '\u{ad}'
                | '\u{61c}'
                | '\u{115f}'
                | '\u{1160}'
                | '\u{180e}'
                | '\u{200b}'
                | '\u{200c}'
                | '\u{200e}'..='\u{200f}'
                | '\u{2028}'..='\u{202e}'
                | '\u{2060}'..='\u{206f}'
                | '\u{3164}'
                | '\u{feff}'
                | '\u{ffa0}'
                | '\u{fff9}'..='\u{fffb}'
                | '\u{e0000}'..='\u{e007f}'
        )
}
