use super::*;
use leon_core::screen::{BgRect, Span};

#[test]
fn colours_fall_back_without_truecolor() {
    let (tc, plain) = (Colors { truecolor: true }, Colors { truecolor: false });
    assert_eq!(tc.rgb(0xf3a747), Color::Rgb(0xf3, 0xa7, 0x47));
    assert_eq!(tc.pane(0x123456), Color::Rgb(0x12, 0x34, 0x56));
    // the theme's ANSI colours are the host's own palette, by index
    assert_eq!(plain.pane(DARK.ansi[1]), Color::Indexed(1));
    assert_eq!(plain.pane(DARK.ansi[12]), Color::Indexed(12));
    assert_eq!(tc.pane(DARK.ansi[1]), tc.rgb(DARK.ansi[1]), "truecolor keeps the theme's exact colour");
    // the default fg/bg are the host's defaults either way
    assert_eq!(plain.pane(DARK.fg), Color::Reset);
    assert_eq!(tc.pane(DARK.bg), Color::Reset);
    // other colours: the nearest xterm-256 entry
    assert_eq!(nearest_256(0xff0000), 196);
    assert_eq!(nearest_256(0x000000), 16);
    assert_eq!(nearest_256(0xffffff), 231);
    assert_eq!(nearest_256(0x808080), 244, "a mid gray is on the gray ramp");
    assert_eq!(plain.rgb(0xff0000), Color::Indexed(196));
}

#[test]
fn a_screen_is_painted_clipped_to_its_pane() {
    let span = |col: usize, text: &str, fg: u32| Span {
        col,
        cells: text.chars().count(),
        text: text.into(),
        fg,
        bold: false,
        italic: false,
        underline: false,
        strikeout: false,
    };
    let f = Frame {
        lines: 3,
        cols: 10,
        rows: vec![
            vec![span(0, "hello", DARK.fg), span(6, "wide-row", 0xff0000)],
            vec![],
            vec![span(0, "gone", DARK.fg)],
        ],
        backgrounds: vec![BgRect { line: 1, col: 2, len: 20, color: 0x00ff00 }],
        ..Frame::default()
    };
    let mut buf = Buffer::empty(Rect::new(0, 0, 12, 4));
    let area = Rect::new(1, 1, 8, 2);
    screen(&mut buf, area, &f, Colors { truecolor: true });
    let row = |y: u16| (0..12).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>();
    assert_eq!(row(1), " hello wi   ", "clipped at the pane's right edge");
    assert_eq!(buf[(7, 1)].fg, Color::Rgb(0xff, 0, 0));
    assert_eq!(buf[(1, 1)].fg, Color::Reset);
    assert_eq!(buf[(3, 2)].bg, Color::Rgb(0, 0xff, 0));
    assert_eq!(buf[(9, 2)].bg, Color::Reset, "backgrounds clip too");
    assert_eq!(row(3), "            ", "rows below the pane aren't drawn");
}
