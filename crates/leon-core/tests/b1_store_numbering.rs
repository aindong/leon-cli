//! Scrollback B1, criterion 1: history past alacritty's window moves into leon's store, and a
//! line keeps its number and text as it moves (docs/architecture/protocol-and-clients.md).

use std::time::{Duration, Instant};

use alacritty_terminal::grid::Dimensions;
use leon_core::screen::row_text;
use leon_core::terminal::{Spec, Terminal};
use leon_core::theme::DARK;

/// alacritty's rows past the screen: its window, never more.
const MOST_IN_ALACRITTY: usize = 1000;

fn shown(bytes: &[u8]) -> Terminal {
    let spec = Spec { command: vec!["/bin/sleep".into(), "60".into()], cwd: None, env: vec![] };
    Terminal::spawn_showing(spec, bytes, |_| {}).unwrap()
}

fn numbered(from: usize, to: usize) -> String {
    (from..to).map(|i| format!("{i}\r\n")).collect()
}

/// Wait for the pause in output that moves history past alacritty's window into the store.
fn settled(t: &Terminal) {
    let end = Instant::now() + Duration::from_secs(10);
    while t.with_term(|t| t.grid().history_size()) > MOST_IN_ALACRITTY {
        assert!(Instant::now() < end, "history never moved into the store");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn line(t: &Terminal, line: usize) -> Option<String> {
    let l = t.lines(line, 1, &DARK);
    l.rows.iter().find(|r| r.line == line).map(|r| row_text(&r.spans))
}

#[test]
fn ten_thousand_lines_keep_their_numbers_with_most_of_them_in_the_store() {
    let t = shown(numbered(0, 30000).as_bytes());
    let l = t.lines(0, 0, &DARK);
    assert_eq!((l.scrolled, l.history), (29977, 10000), "the numbering is today's");
    let alacritty = t.with_term(|t| t.grid().history_size());
    assert!(alacritty <= MOST_IN_ALACRITTY, "alacritty keeps only its window: {alacritty} rows");
    for n in (19977..30000).step_by(7).chain([19977, 25000, 29976, 29977, 29999]) {
        assert_eq!(line(&t, n).as_deref(), Some(n.to_string().as_str()), "line {n}");
    }
    assert_eq!(line(&t, 19976), None, "older than the cap");
    // a range across the store, alacritty's rows and the screen, in one call
    let oldest_in_alacritty = 29977 - alacritty;
    let from = oldest_in_alacritty - 500;
    let rows = t.lines(from, 1000, &DARK).rows;
    let got: Vec<(usize, String)> =
        rows.iter().filter(|r| r.line < 30000).map(|r| (r.line, row_text(&r.spans))).collect();
    let want: Vec<(usize, String)> = (from..30000).map(|n| (n, n.to_string())).collect();
    assert_eq!(got, want, "a range across the boundary");
}

#[test]
fn a_line_read_before_it_moves_reads_the_same_after() {
    // `cat` prints what it is sent: the lines arrive in two parts, the second pushing the first's
    // lines out of alacritty's window into the store
    let script = "stty raw -echo; printf 'ready\\r\\n'; exec cat";
    let spec = Spec { command: vec!["sh".into(), "-c".into(), script.into()], cwd: None, env: vec![] };
    let t = Terminal::spawn(spec, |_| {}).unwrap();
    let wait = |what: &str| {
        let end = Instant::now() + Duration::from_secs(20);
        while !t.text(true).ends_with(what) {
            assert!(Instant::now() < end, "never printed {what}");
            std::thread::sleep(Duration::from_millis(20));
        }
    };
    // `ready` comes after stty: from then on input isn't echoed, only printed by cat
    wait("ready");
    t.write(numbered(0, 600).as_bytes()).unwrap();
    wait("599");
    // history only: the screen's rows below the cursor fill with the next lines
    let history_end = t.lines(0, 0, &DARK).scrolled;
    let before: Vec<(usize, Option<String>)> = (0..history_end).map(|n| (n, line(&t, n))).collect();
    assert!(before.iter().any(|(_, l)| l.as_deref() == Some("300")), "line 300 is kept: {:?}", &before[..5]);
    assert_eq!(t.with_term(|t| t.grid().history_size()), t.lines(0, 0, &DARK).history, "nothing stored yet");
    t.write(numbered(600, 5000).as_bytes()).unwrap();
    wait("4999");
    settled(&t);
    let l = t.lines(0, 0, &DARK);
    let alacritty = t.with_term(|t| t.grid().history_size());
    assert!(
        l.history > alacritty,
        "the first lines moved into the store ({} kept, {alacritty} in alacritty)",
        l.history
    );
    let oldest_in_alacritty = l.scrolled - alacritty;
    for (n, text) in before.into_iter().filter(|(n, _)| *n < oldest_in_alacritty) {
        assert_eq!(line(&t, n), text, "line {n} kept its number and text as it moved");
    }
}
