//! Painting a terminal's grid from the screen the server pushed: cell size, the fit, the frame.

use super::*;

pub(super) fn terminal_font(bold: bool, italic: bool) -> Font {
    Font {
        weight: if bold { FontWeight::BOLD } else { FontWeight::NORMAL },
        style: if italic { FontStyle::Italic } else { FontStyle::Normal },
        ..font(FONT_FAMILY)
    }
}

/// A terminal cell's (width, height) at `pt`.
pub(super) fn cell_size(window: &Window, pt: f32) -> (Pixels, Pixels) {
    let ts = window.text_system();
    let font_id = ts.resolve_font(&terminal_font(false, false));
    let cell_w = ts.advance(font_id, px(pt), 'm').map(|s| s.width).ok().filter(|w| *w > px(0.)).unwrap_or(px(8.));
    (cell_w, (px(pt) * LINE_HEIGHT).round())
}

/// The (rows, cols) that fit in (w, h): at least what `Terminal::resize` clamps to, or every
/// frame would ask for a resize again.
pub(super) fn cells_in(w: Pixels, h: Pixels, cell_w: Pixels, line_h: Pixels) -> (u16, u16) {
    let cells = |len: Pixels, unit: Pixels, least: u16| ((len / unit).floor().max(0.) as u16).max(least);
    (cells(h, line_h, 1), cells(w, cell_w, 2))
}

/// Paints one block's screen as the server last pushed it. Prepaint asks the server to fit the
/// grid to the bounds (resizing the PTY when rows or columns change); paint draws the frame.
pub(super) fn terminal_canvas(
    app: Rc<Remote>,
    (sid, block_id): (String, String),
    active: bool,
    font_size: f32,
    stats: Rc<RefCell<Stats>>,
) -> impl IntoElement {
    canvas(
        move |bounds, window, _| {
            let (cell_w, line_h) = cell_size(window, font_size);
            let (rows, cols) = cells_in(bounds.size.width, bounds.size.height, cell_w, line_h);
            app.fit(&sid, &block_id, rows, cols);
            let frame = app.model().screens.get(&block_id).cloned().unwrap_or_default();
            (frame, cell_w, line_h)
        },
        move |bounds, (frame, cell_w, line_h), window, cx| {
            paint_frame(&frame, &DARK, bounds, cell_w, line_h, font_size, active, window, cx);
            stats.borrow_mut().painted();
        },
    )
    .size_full()
}

#[allow(clippy::too_many_arguments)]
pub(super) fn paint_frame(
    f: &Frame,
    t: &Theme,
    bounds: Bounds<Pixels>,
    cell_w: Pixels,
    line_h: Pixels,
    font_size: f32,
    active: bool,
    window: &mut Window,
    cx: &mut App,
) {
    let at =
        |line: usize, col: usize| point(bounds.origin.x + cell_w * col as f32, bounds.origin.y + line_h * line as f32);
    for r in &f.backgrounds {
        window.paint_quad(fill(Bounds::new(at(r.line, r.col), size(cell_w * r.len as f32, line_h)), rgb(r.color)));
    }
    let mark = f.cursor.as_ref().map(|c| {
        let w = cell_w * if c.wide { 2. } else { 1. };
        let origin = at(c.line, c.col);
        match c.shape {
            CursorShape::Beam => Mark::Bar(Bounds::new(origin, size(px(2.), line_h))),
            CursorShape::Underline => {
                Mark::Bar(Bounds::new(point(origin.x, origin.y + line_h - px(2.)), size(w, px(2.))))
            }
            CursorShape::Block if active => Mark::Block(Bounds::new(origin, size(w, line_h))),
            _ => Mark::Hollow(Bounds::new(origin, size(w, line_h))), // also any block in an inactive window
        }
    });
    let mut paint_text =
        |text: &str, line: usize, col: usize, color: u32, s: Option<&render::Span>, window: &mut Window| {
            let (bold, italic, underline, strike) =
                s.map_or((false, false, false, false), |s| (s.bold, s.italic, s.underline, s.strikeout));
            let run = TextRun {
                len: text.len(),
                font: terminal_font(bold, italic),
                color: rgb(color).into(),
                background_color: None,
                underline: underline.then(|| UnderlineStyle { thickness: px(1.), color: None, wavy: false }),
                strikethrough: strike.then(|| StrikethroughStyle { thickness: px(1.), color: None }),
            };
            // Pin glyph i to column i, so a fallback font's advance can't push text off the grid.
            let grid = s.filter(|s| s.cells == s.text.chars().count()).map(|_| cell_w);
            let shaped =
                window.text_system().shape_line(SharedString::from(text.to_string()), px(font_size), &[run], grid);
            let _ = shaped.paint(at(line, col), line_h, window, cx);
        };
    // A solid block cursor sits under the text; its cell's text is redrawn in the background color.
    if let Some(Mark::Block(b)) = mark {
        window.paint_quad(fill(b, rgb(t.cursor)));
    }
    for (line, spans) in f.rows.iter().enumerate() {
        for s in spans {
            paint_text(&s.text, line, s.col, s.fg, Some(s), window);
        }
    }
    match (mark, &f.cursor) {
        (Some(Mark::Block(_)), Some(c)) if !c.text.trim().is_empty() => {
            paint_text(&c.text, c.line, c.col, t.bg, None, window)
        }
        (Some(Mark::Bar(b)), _) => window.paint_quad(fill(b, rgb(t.cursor))),
        (Some(Mark::Hollow(b)), _) => window.paint_quad(outline(b, rgb(t.cursor), gpui::BorderStyle::Solid)),
        _ => {}
    }
}

/// How the cursor is drawn.
enum Mark {
    /// Filled, under the text.
    Block(Bounds<Pixels>),
    /// A beam or underline bar, over the text.
    Bar(Bounds<Pixels>),
    /// An outline: a hollow-block cursor, or the cursor of an inactive window.
    Hollow(Bounds<Pixels>),
}
