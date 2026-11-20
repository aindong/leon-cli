//! Review (security): the check This Mac's server makes of a link's address, shared so the app
//! makes it of a host's answer too.
use leon_core::search::{clean_uri, MAX_URI};

#[test]
fn a_plain_address_is_clean() {
    assert!(clean_uri("https://example.com/page?a=1#b"));
}

#[test]
fn hidden_control_or_huge_is_not() {
    assert!(!clean_uri("https://example.com/\u{202e}gpj.exe"), "a bidi control");
    assert!(!clean_uri("https://example.com/\nrm -rf ~"), "a newline");
    assert!(!clean_uri("https://example.com/\u{e0041}"), "a tag character");
    assert!(!clean_uri(&format!("https://example.com/{}", "a".repeat(MAX_URI))), "over MAX_URI");
}
