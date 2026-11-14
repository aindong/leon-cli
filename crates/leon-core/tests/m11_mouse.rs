//! M11 criterion 3: mouse reports for each mode and encoding (`leon_core::mouse`).

use leon_core::mouse::{encode, Action, Button, Event, Mode, Mods, Report, X10Scanner};

fn mode(report: Option<Report>) -> Mode {
    Mode { report, ..Mode::default() }
}

fn sgr(report: Report) -> Mode {
    Mode { report: Some(report), sgr: true, ..Mode::default() }
}

fn ev(action: Action, button: Option<Button>, line: usize, col: usize) -> Event {
    Event { action, button, line, col, mods: Mods::default() }
}

fn press(line: usize, col: usize) -> Event {
    ev(Action::Press, Some(Button::Left), line, col)
}

fn bytes(e: &Event, m: &Mode) -> Option<String> {
    encode(e, m).map(|b| String::from_utf8_lossy(&b).into_owned())
}

#[test]
fn sgr_reports_presses_releases_and_modifiers() {
    let m = sgr(Report::Click);
    assert_eq!(bytes(&press(4, 9), &m).as_deref(), Some("\x1b[<0;10;5M"), "1-based col;row");
    let release = ev(Action::Release, Some(Button::Left), 4, 9);
    assert_eq!(bytes(&release, &m).as_deref(), Some("\x1b[<0;10;5m"), "release keeps the button, ends in m");
    let right = ev(Action::Press, Some(Button::Right), 0, 0);
    assert_eq!(bytes(&right, &m).as_deref(), Some("\x1b[<2;1;1M"));
    let middle = ev(Action::Press, Some(Button::Middle), 0, 0);
    assert_eq!(bytes(&middle, &m).as_deref(), Some("\x1b[<1;1;1M"));
    let mut held = press(0, 0);
    held.mods = Mods { shift: true, alt: true, ctrl: true };
    assert_eq!(bytes(&held, &m).as_deref(), Some("\x1b[<28;1;1M"), "shift 4 + alt 8 + ctrl 16");
    assert_eq!(bytes(&press(600, 1500), &m).as_deref(), Some("\x1b[<0;1501;601M"), "SGR has no limit");
}

#[test]
fn legacy_encoding_is_three_offset_bytes() {
    let m = mode(Some(Report::Click));
    assert_eq!(encode(&press(4, 9), &m), Some(b"\x1b[M\x20\x2a\x25".to_vec()), "32+b, 33+col, 33+row");
    let release = ev(Action::Release, Some(Button::Left), 4, 9);
    assert_eq!(encode(&release, &m), Some(b"\x1b[M\x23\x2a\x25".to_vec()), "release is button 3");
    assert_eq!(encode(&press(0, 222), &m), Some(vec![0x1b, b'[', b'M', 32, 255, 33]), "column 223 is the last");
    assert_eq!(encode(&press(0, 223), &m), None, "past 223 can't be said");
}

#[test]
fn utf8_encoding_writes_coordinates_as_characters() {
    let m = Mode { report: Some(Report::Click), utf8: true, ..Mode::default() };
    assert_eq!(bytes(&press(0, 300), &m).as_deref(), Some("\x1b[M \u{14d}!"), "33+300 = U+014D");
    assert!(encode(&press(0, 3000), &m).is_none(), "past what two UTF-8 bytes hold");
    let both = Mode { sgr: true, ..m };
    assert_eq!(bytes(&press(0, 300), &both).as_deref(), Some("\x1b[<0;301;1M"), "SGR wins over UTF-8");
}

#[test]
fn each_mode_reports_only_what_it_asks_for() {
    let drag = ev(Action::Move, Some(Button::Left), 1, 1);
    let hover = ev(Action::Move, None, 1, 1);
    assert_eq!(bytes(&drag, &sgr(Report::Click)), None, "1000: no motion at all");
    assert_eq!(bytes(&drag, &sgr(Report::Drag)).as_deref(), Some("\x1b[<32;2;2M"), "1002: drags, +32");
    assert_eq!(bytes(&hover, &sgr(Report::Drag)), None, "1002: no motion without a button");
    assert_eq!(bytes(&hover, &sgr(Report::Motion)).as_deref(), Some("\x1b[<35;2;2M"), "1003: every move");
    assert_eq!(bytes(&press(0, 0), &mode(None)), None, "no mode, no report");
}

#[test]
fn x10_mode_reports_presses_only_without_modifiers() {
    let m = mode(Some(Report::X10));
    let mut p = press(0, 0);
    p.mods.ctrl = true;
    assert_eq!(encode(&p, &m), Some(b"\x1b[M\x20\x21\x21".to_vec()), "no modifier bits");
    assert_eq!(encode(&ev(Action::Release, Some(Button::Left), 0, 0), &m), None, "no release");
    assert_eq!(encode(&ev(Action::Move, Some(Button::Left), 0, 0), &m), None, "no drag");
}

#[test]
fn wheel_is_buttons_64_and_65_or_arrows_on_the_alt_screen() {
    let up = ev(Action::Press, Some(Button::WheelUp), 2, 3);
    let down = ev(Action::Press, Some(Button::WheelDown), 2, 3);
    assert_eq!(bytes(&up, &sgr(Report::Click)).as_deref(), Some("\x1b[<64;4;3M"));
    assert_eq!(bytes(&down, &sgr(Report::Click)).as_deref(), Some("\x1b[<65;4;3M"));
    let alt = Mode { alt_scroll: true, ..Mode::default() };
    assert_eq!(bytes(&up, &alt).as_deref(), Some("\x1b[A"), "alternate scroll: an arrow per notch");
    assert_eq!(bytes(&down, &alt).as_deref(), Some("\x1b[B"));
    let app = Mode { alt_scroll: true, app_cursor: true, ..Mode::default() };
    assert_eq!(bytes(&up, &app).as_deref(), Some("\x1bOA"), "in the program's cursor-key mode");
    assert_eq!(bytes(&up, &mode(None)), None, "the primary screen scrolls the view instead");
}

#[test]
fn the_x10_scanner_follows_set_and_reset_even_split() {
    let mut s = X10Scanner::default();
    s.feed(b"hello \x1b[?9h");
    assert!(s.on());
    s.feed(b"\x1b[?1000;9");
    assert!(s.on(), "nothing decided mid-sequence");
    s.feed(b"l more");
    assert!(!s.on(), "reset in a list, split across reads");
    s.feed(b"\x1b[?19h\x1b[?90h");
    assert!(!s.on(), "19 and 90 aren't 9");
    s.feed(b"\x1b[?1;9h");
    assert!(s.on());
    s.feed(b"\x1bc");
    assert!(!s.on(), "a full reset clears it");
    s.feed(&[b"\x1b[?".as_slice(), [b'1'; 100_000].as_slice()].concat());
    s.feed(b";9h");
    assert!(!s.on(), "an endless parameter is dropped, not buffered");
}
