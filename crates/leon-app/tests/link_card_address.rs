//! The card shows a link's address plainly: hidden characters stripped, an encoded address also
//! shown as what it opens, and a huge address cut.
use leon_app::view::link_card::link_card_lines;

#[test]
fn a_plain_address_is_one_line() {
    assert_eq!(link_card_lines("https://example.com/page"), vec!["https://example.com/page".to_string()]);
}

#[test]
fn hidden_characters_are_stripped() {
    let lines = link_card_lines("https://example.com/\u{202e}gpj.exe");
    assert_eq!(lines, vec!["https://example.com/gpj.exe".to_string()]);
}

#[test]
fn an_encoded_address_also_shows_what_it_opens() {
    let lines = link_card_lines("https://ex%61mple.com/%2e%2e/admin");
    let want = ["https://ex%61mple.com/%2e%2e/admin", "opens https://ex%61mple.com/../admin", "host example.com"];
    assert_eq!(lines, want.map(String::from));
}

#[test]
fn a_huge_address_is_cut() {
    let long = format!("https://example.com/{}", "a".repeat(10_000));
    let lines = link_card_lines(&long);
    assert_eq!(lines[1], "host example.com", "a cut address shows its host");
    assert!(lines[0].chars().count() <= 513 && lines[0].ends_with('…'), "{} chars", lines[0].chars().count());
}
