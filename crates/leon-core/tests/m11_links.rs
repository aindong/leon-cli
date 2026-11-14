//! M11 criterion 5: the link under the mouse (`Terminal::link_at`) and which links open without
//! asking (`search::needs_confirm`).

use leon_core::search::{needs_confirm, Pos};
use leon_core::terminal::{Spec, Terminal};

fn shown(bytes: &[u8]) -> Terminal {
    let spec = Spec { command: vec!["/bin/sleep".into(), "60".into()], cwd: None, env: vec![] };
    Terminal::spawn_showing(spec, bytes, |_| {}).unwrap()
}

fn at(line: usize, col: usize) -> Pos {
    Pos { line, col }
}

#[test]
fn an_osc8_hyperlink() {
    let t = shown(b"x \x1b]8;;https://example.com/a?b=1\x1b\\link\x1b]8;;\x1b\\ y");
    let l = t.link_at(at(0, 3)).expect("the linked text");
    assert_eq!(l.uri, "https://example.com/a?b=1");
    assert_eq!((l.start, l.end), (at(0, 2), at(0, 5)));
    assert!(t.link_at(at(0, 0)).is_none(), "plain text has no link");
}

#[test]
fn a_detected_url_across_a_wrap() {
    let path = "p".repeat(90);
    let url = format!("https://example.com/{path}");
    let t = shown(format!("see {url} now\r\n").as_bytes());
    for hover in [at(0, 10), at(1, 5)] {
        let l = t.link_at(hover).expect("a URL on either row");
        assert_eq!(l.uri, url);
        assert_eq!((l.start, l.end), (at(0, 4), at(1, url.len() + 4 - 81)));
    }
    assert!(t.link_at(at(1, 36)).is_none(), "the word after it");
}

#[test]
fn trailing_punctuation_is_not_part_of_a_url() {
    let t = shown(b"(see https://example.com/x). And https://a.org/b_(c), too\r\n");
    assert_eq!(t.link_at(at(0, 10)).unwrap().uri, "https://example.com/x");
    assert_eq!(t.link_at(at(0, 35)).unwrap().uri, "https://a.org/b_(c)", "balanced parentheses stay");
}

#[test]
fn a_uri_with_hidden_characters_is_refused() {
    let t = shown("\x1b]8;;https://example.com/\u{202e}gpj.exe\x1b\\link\x1b]8;;\x1b\\".as_bytes());
    assert!(t.link_at(at(0, 1)).is_none(), "a bidi override in the URI");
}

#[test]
fn only_web_links_open_without_asking() {
    assert!(!needs_confirm("https://example.com"));
    assert!(!needs_confirm("http://localhost:3000/x"));
    assert!(!needs_confirm("HTTPS://EXAMPLE.COM"));
    for uri in ["file:///etc/passwd", "mailto:a@b.c", "vscode://open?x", "ssh://host", "javascript:alert(1)"] {
        assert!(needs_confirm(uri), "{uri}");
    }
}
