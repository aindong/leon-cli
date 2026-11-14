//! Review proof (security): a URL followed by a huge run of `)` (one wrapped logical line) can't
//! make the link lookup take long while it holds the terminal's lock.

use std::time::Duration;

use leon_core::search::{decoded, Pos};
use leon_core::terminal::{Spec, Terminal};

#[test]
fn a_url_with_a_long_run_of_closing_parens_is_bounded() {
    let mut bytes = b"https://a/".to_vec();
    bytes.extend(std::iter::repeat_n(b')', 700_000));
    let spec = Spec { command: vec!["/bin/sleep".into(), "60".into()], cwd: None, env: vec![] };
    let t = Terminal::spawn_showing(spec, &bytes, |_| {}).unwrap();
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_secs(5));
        eprintln!("link_at took too long");
        std::process::exit(101);
    });
    let l = t.link_at(Pos { line: t.scrolled() - t.lines(0, 0, &leon_core::theme::DARK).history, col: 3 });
    assert!(l.is_none_or(|l| l.uri.len() <= leon_core::search::MAX_URI * 2));
}

#[test]
fn a_prompt_shows_where_escapes_lead() {
    assert_eq!(
        decoded("file:///a/doc.pdf%2F..%2F..%2FApplications%2FX.app"),
        "file:///a/doc.pdf/../../Applications/X.app"
    );
    assert_eq!(decoded("https://x/%E2%80%AEevil"), "https://x/evil", "a decoded bidi override is dropped");
    assert_eq!(decoded("100%"), "100%");
}
