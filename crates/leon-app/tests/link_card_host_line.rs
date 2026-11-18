//! Review (security): the card never lets a link's user-info, an escaped or lookalike host, or a
//! cut hide where it goes: only what follows the host is decoded, and the host shows on a line of
//! its own when it could mislead.
use leon_app::view::link_card::link_card_lines;

#[test]
fn an_encoded_userinfo_never_reads_as_the_host() {
    let lines = link_card_lines("https://accounts.google.com%2Fsignin@evil.example/");
    assert!(!lines.iter().any(|l| l.contains("opens https://accounts.google.com/")), "{lines:?}");
    assert!(lines.contains(&"host evil.example".to_string()), "{lines:?}");
}

#[test]
fn a_lookalike_host_shows_its_punycode() {
    let lines = link_card_lines("https://\u{430}pple.com/");
    assert!(lines.contains(&"host xn--pple-43d.com".to_string()), "{lines:?}");
}

#[test]
fn an_escaped_host_shows_what_it_is() {
    let lines = link_card_lines("https://ex%61mple.com:8443/%2e%2e/admin");
    assert_eq!(
        lines,
        vec![
            "https://ex%61mple.com:8443/%2e%2e/admin".to_string(),
            "opens https://ex%61mple.com:8443/../admin".to_string(),
            "host example.com".to_string(),
        ]
    );
}

#[test]
fn a_cut_address_still_shows_its_host() {
    let uri = format!("https://accounts.google.com.{}@evil.example/", "a".repeat(3000));
    let lines = link_card_lines(&uri);
    assert!(lines.contains(&"host evil.example".to_string()), "{} lines", lines.len());
}

#[test]
fn a_plain_web_or_file_link_has_no_host_line() {
    assert_eq!(link_card_lines("https://example.com/page").len(), 1);
    assert_eq!(link_card_lines("file:///etc/hosts").len(), 1);
}

#[test]
fn a_backslash_ends_the_host_as_a_browser_reads_it() {
    let lines = link_card_lines("https://evil.example\\@trusted.example/");
    assert!(lines.contains(&"host evil.example".to_string()), "{lines:?}");
}

#[test]
fn a_huge_host_is_cut_too() {
    let uri = format!("https://user@{}.example/", "a".repeat(3000));
    let lines = link_card_lines(&uri);
    assert!(
        lines.iter().all(|l| l.chars().count() <= 520),
        "every line is cut: {:?}",
        lines.iter().map(|l| l.len()).collect::<Vec<_>>()
    );
}
