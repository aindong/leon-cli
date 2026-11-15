//! Report grammar: every rule and limit in docs/agents.md.

use base64::Engine as _;
use leon_core::program_status::{parse, Ignored, Kind, Message, Piece, Record, Scanner, State};

fn rec(state: State) -> Record {
    Record { state, kind: None, progress: None, app: None, title: None, msg: None }
}

fn set(id: Option<&str>, record: Record) -> Result<Message, Ignored> {
    Ok(Message::Set { id: id.map(String::from), record })
}

fn b64(s: &str) -> String {
    base64::engine::general_purpose::STANDARD.encode(s)
}

#[test]
fn grammar_rules() {
    let seg32 = "s".repeat(32);
    let seg33 = "s".repeat(33);
    let deep8 = "a/b/c/d/e/f/g/h";
    let deep9 = "a/b/c/d/e/f/g/h/i";
    let id128 = format!("{0}/{0}/{0}/{1}", "x".repeat(32), "y".repeat(29));
    let id129 = format!("{0}/{0}/{0}/{1}", "x".repeat(32), "y".repeat(30));
    let cases: Vec<(String, Result<Message, Ignored>)> = vec![
        ("state=idle".into(), set(None, rec(State::Idle))),
        ("?".into(), Ok(Message::Query)),
        (" ? ".into(), Ok(Message::Query)),
        // a query with pairs is not a query, so report content is never echoed into a reply
        ("?:title=aGk=".into(), Err(Ignored::NoState)),
        ("?:state=idle".into(), set(None, rec(State::Idle))),
        // whitespace trimmed around the payload, pairs, keys and values
        (" state = working : progress = 50 ".into(), set(None, Record { progress: Some(50), ..rec(State::Working) })),
        ("state=idle\t".into(), set(None, rec(State::Idle))),
        // malformed pair skipped
        ("state=working:bogus".into(), set(None, rec(State::Working))),
        ("state=working:Progress=5".into(), set(None, rec(State::Working))),
        ("state=working::".into(), set(None, rec(State::Working))),
        ("state=wor king".into(), Err(Ignored::NoState)),
        ("state=idle:\u{e9}=1".into(), set(None, rec(State::Idle))),
        // unknown keys ignored
        ("state=working:foo=bar".into(), set(None, rec(State::Working))),
        // last duplicate wins, among well-formed pairs
        ("state=idle:state=done".into(), set(None, rec(State::Done))),
        ("state=done:state=w@t".into(), set(None, rec(State::Done))),
        ("state=done:state=nope".into(), Err(Ignored::BadState)),
        ("state=working:state=bogus".into(), Err(Ignored::BadState)),
        ("state=bogus:state=working".into(), set(None, rec(State::Working))),
        // state required and known
        ("".into(), Err(Ignored::NoState)),
        ("progress=5".into(), Err(Ignored::NoState)),
        ("state=".into(), Err(Ignored::BadState)),
        ("state=Working".into(), Err(Ignored::BadState)),
        // clear
        ("state=clear".into(), Ok(Message::Clear { id: None })),
        ("state=clear:id=a/b".into(), Ok(Message::Clear { id: Some("a/b".into()) })),
        ("state=clear:id=".into(), Err(Ignored::BadId)),
        // id grammar and limits; a bad id never falls back to root
        ("state=idle:id=a/b".into(), set(Some("a/b"), rec(State::Idle))),
        ("state=idle:id=A-z_0.9+".into(), set(Some("A-z_0.9+"), rec(State::Idle))),
        ("state=idle:id=".into(), Err(Ignored::BadId)),
        ("state=idle:id=a//b".into(), Err(Ignored::BadId)),
        ("state=idle:id=/a".into(), Err(Ignored::BadId)),
        ("state=idle:id=a/".into(), Err(Ignored::BadId)),
        ("state=idle:id=a,b".into(), Err(Ignored::BadId)),
        ("state=idle:id=a=b".into(), Err(Ignored::BadId)),
        (format!("state=idle:id={seg32}"), set(Some(&seg32), rec(State::Idle))),
        (format!("state=idle:id={seg33}"), Err(Ignored::BadId)),
        (format!("state=idle:id={deep8}"), set(Some(deep8), rec(State::Idle))),
        (format!("state=idle:id={deep9}"), Err(Ignored::BadId)),
        (format!("state=idle:id={id128}"), set(Some(&id128), rec(State::Idle))),
        (format!("state=idle:id={id129}"), Err(Ignored::BadId)),
        // kind only on blocked, and only known kinds
        (
            "state=blocked:kind=permission".into(),
            set(None, Record { kind: Some(Kind::Permission), ..rec(State::Blocked) }),
        ),
        ("state=blocked:kind=question".into(), set(None, Record { kind: Some(Kind::Question), ..rec(State::Blocked) })),
        ("state=blocked:kind=auth".into(), set(None, Record { kind: Some(Kind::Auth), ..rec(State::Blocked) })),
        ("state=blocked:kind=nope".into(), set(None, rec(State::Blocked))),
        ("state=working:kind=auth".into(), set(None, rec(State::Working))),
        // progress 0-100, only on working/blocked
        ("state=working:progress=0".into(), set(None, Record { progress: Some(0), ..rec(State::Working) })),
        ("state=blocked:progress=100".into(), set(None, Record { progress: Some(100), ..rec(State::Blocked) })),
        ("state=working:progress=101".into(), set(None, rec(State::Working))),
        ("state=working:progress=+5".into(), set(None, rec(State::Working))),
        ("state=working:progress=-1".into(), set(None, rec(State::Working))),
        ("state=working:progress=5.5".into(), set(None, rec(State::Working))),
        ("state=working:progress=".into(), set(None, rec(State::Working))),
        ("state=done:progress=5".into(), set(None, rec(State::Done))),
        // app
        ("state=idle:app=claude".into(), set(None, Record { app: Some("claude".into()), ..rec(State::Idle) })),
        ("state=idle:app=a,b".into(), set(None, rec(State::Idle))),
        ("state=idle:app=a/b".into(), set(None, rec(State::Idle))),
        ("state=idle:app=".into(), set(None, rec(State::Idle))),
        // title / msg: base64 UTF-8, padded or not
        (
            format!("state=done:title={}", b64("hello")),
            set(None, Record { title: Some("hello".into()), ..rec(State::Done) }),
        ),
        ("state=done:title=aGVsbG8".into(), set(None, Record { title: Some("hello".into()), ..rec(State::Done) })),
        (
            format!("state=error:msg={}", b64("héllo")),
            set(None, Record { msg: Some("héllo".into()), ..rec(State::Error) }),
        ),
        ("state=done:title=a".into(), set(None, rec(State::Done))),
        ("state=done:title=/w==".into(), set(None, rec(State::Done))),
        ("state=done:title=".into(), set(None, rec(State::Done))),
    ];
    for (payload, want) in cases {
        assert_eq!(parse(payload.as_bytes()), want, "payload {payload:?}");
    }
}

#[test]
fn non_utf8_payload_bytes_are_malformed_pairs() {
    assert_eq!(parse(b"state=idle:\xff\xfe=1:x=\x80"), set(None, rec(State::Idle)));
}

#[test]
fn limits_discard_the_whole_report() {
    let key16 = "k".repeat(16);
    let key17 = "k".repeat(17);
    let app32 = "a".repeat(32);
    let app33 = "a".repeat(33);
    let title192 = b64(&"t".repeat(192));
    let title193 = b64(&"t".repeat(193));
    let msg2048 = b64(&"m".repeat(2048));
    let msg2049 = b64(&"m".repeat(2049));
    assert_eq!(msg2049.len(), 2732, "2049 bytes still encode to 2732, so the decoded limit is what catches it");
    let cases: Vec<(String, Result<Message, Ignored>)> = vec![
        (format!("state=idle:{key16}=1"), set(None, rec(State::Idle))),
        (format!("state=idle:{key17}=1"), Err(Ignored::TooLong("key"))),
        (format!("state=idle:app={app32}"), set(None, Record { app: Some(app32.clone()), ..rec(State::Idle) })),
        (format!("state=idle:app={app33}"), Err(Ignored::TooLong("app"))),
        (
            format!("state=done:title={title192}"),
            set(None, Record { title: Some("t".repeat(192)), ..rec(State::Done) }),
        ),
        (format!("state=done:title={title193}"), Err(Ignored::TooLong("title"))),
        (format!("state=done:title={}", "A".repeat(257)), Err(Ignored::TooLong("title"))),
        (format!("state=done:msg={msg2048}"), set(None, Record { msg: Some("m".repeat(2048)), ..rec(State::Done) })),
        (format!("state=done:msg={msg2049}"), Err(Ignored::TooLong("msg"))),
        (format!("state=done:msg={}", "A".repeat(2733)), Err(Ignored::TooLong("msg"))),
        // a limit is checked even when a later duplicate would replace the value
        (format!("state=idle:app={app33}:app=ok"), Err(Ignored::TooLong("app"))),
    ];
    for (payload, want) in cases {
        assert_eq!(parse(payload.as_bytes()), want, "payload of {} bytes", payload.len());
    }
}

#[test]
fn sequence_limit_is_4096_bytes_including_esc_and_terminator() {
    // ESC ] 7501 ; = 7 bytes, BEL = 1, ESC \ = 2. Fed byte by byte; returns (report lengths, pass-through).
    let scan = |seq: &str| {
        let (mut got, mut pass) = (Vec::new(), Vec::new());
        let mut scanner = Scanner::default();
        for byte in format!("{seq}after").as_bytes().chunks(1) {
            scanner.feed(byte, |p| match p {
                Piece::Report(r) => got.push(r.len()),
                Piece::Bytes(b) => pass.extend_from_slice(b),
                _ => {}
            });
        }
        (got, String::from_utf8(pass).unwrap())
    };
    let pad = |n: usize| format!("state=idle:x={}", "a".repeat(n - "state=idle:x=".len()));
    let after = || "after".to_string();
    assert_eq!(scan(&format!("\x1b]7501;{}\x07", pad(4088))), (vec![4088], after()));
    assert_eq!(scan(&format!("\x1b]7501;{}\x07", pad(4089))), (vec![], after()));
    assert_eq!(scan(&format!("\x1b]7501;{}\x1b\\", pad(4087))), (vec![4087], after()));
    assert_eq!(scan(&format!("\x1b]7501;{}\x1b\\", pad(4088))), (vec![], after()));
}

#[test]
fn unsafe_text_drops_the_field_and_emoji_survive() {
    let rejected = [
        "a\nb",
        "a\rb",
        "tab\there",
        "\u{7f}",
        "\u{85}",
        "\u{9b}31m",
        "\u{1b}[31m",
        "\u{202e}evil",
        "\u{2066}x",
        "\u{200e}",
        "\u{200f}",
        "\u{61c}",
        "\u{200b}",
        "\u{200c}",
        "\u{2060}",
        "\u{feff}",
        "\u{ad}",
        "\u{2028}",
        "\u{2029}",
        "\u{180e}",
        "\u{3164}",
        "\u{e0041}",
    ];
    for text in rejected {
        for key in ["title", "msg"] {
            let payload = format!("state=done:{key}={}", b64(text));
            assert_eq!(parse(payload.as_bytes()), set(None, rec(State::Done)), "{key} {text:?} must be dropped");
        }
    }
    for text in ["héllo wörld", "👨\u{200d}👩\u{200d}👧", "❤\u{fe0f}", "日本語", "a b"] {
        let payload = format!("state=done:title={}", b64(text));
        let want = set(None, Record { title: Some(text.into()), ..rec(State::Done) });
        assert_eq!(parse(payload.as_bytes()), want, "{text:?} must be kept");
    }
}
