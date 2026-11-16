//! Painting a terminal's grid from the screen the server pushed: cell size, the fit, the frame.

use super::*;

pub(super) fn terminal_font(family: &SharedString, bold: bool, italic: bool) -> Font {
    Font {
        weight: if bold { FontWeight::BOLD } else { FontWeight::NORMAL },
        style: if italic { FontStyle::Italic } else { FontStyle::Normal },
        ..font(family.clone())
    }
}

/// A terminal cell's (width, height) in `family` at `pt`.
pub(super) fn cell_size(window: &Window, family: &SharedString, pt: f32) -> (Pixels, Pixels) {
    let ts = window.text_system();
    let font_id = ts.resolve_font(&terminal_font(family, false, false));
    let cell_w = ts.advance(font_id, px(pt), 'm').map(|s| s.width).ok().filter(|w| *w > px(0.)).unwrap_or(px(8.));
    (cell_w, (px(pt) * LINE_HEIGHT).round())
}

/// The (rows, cols) that fit in (w, h): at least what `Terminal::resize` clamps to, or every
/// frame would ask for a resize again.
pub(super) fn cells_in(w: Pixels, h: Pixels, cell_w: Pixels, line_h: Pixels) -> (u16, u16) {
    let cells = |len: Pixels, unit: Pixels, least: u16| ((len / unit).floor().max(0.) as u16).max(least);
    (cells(h, line_h, 1), cells(w, cell_w, 2))
}

/// What a cell range is drawn with, besides its text.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Mark {
    Selection,
    Found,
    /// The current find match: outlined.
    Current,
    /// The link under the mouse while cmd is held: underlined.
    Link,
}

/// Columns `from..=to` of view row `row`, drawn as `mark`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct CellMark {
    pub(super) row: usize,
    pub(super) from: usize,
    pub(super) to: usize,
    pub(super) mark: Mark,
}

/// Paints one block's view: the screen the server last pushed, or rows of its history when
/// scrolled back, with selection, find and link marks. Prepaint asks the server to fit the grid
/// to the bounds (resizing the PTY when rows or columns change); paint draws it and notes where,
/// for the mouse.
pub(super) fn terminal_canvas(
    app: Rc<Remote>,
    (sid, block_id): (String, String),
    (frame, marks): (Frame, Vec<CellMark>),
    active: bool,
    (family, font_size, theme): (SharedString, f32, Theme),
    (stats, painted): (Rc<RefCell<Stats>>, scroll::PaintedMap),
) -> impl IntoElement {
    let family2 = family.clone();
    canvas(
        move |bounds, window, _| {
            let (cell_w, line_h) = cell_size(window, &family2, font_size);
            let (rows, cols) = cells_in(bounds.size.width, bounds.size.height, cell_w, line_h);
            app.fit(&sid, &block_id, rows, cols);
            (frame, cell_w, line_h, (rows as usize, cols as usize), block_id)
        },
        move |bounds, (frame, cell_w, line_h, size, id), window, cx| {
            let p = scroll::Painted { origin: bounds.origin, cell: (cell_w, line_h), size };
            paint_frame(&frame, &marks, &theme, bounds, (cell_w, line_h), (&family, font_size), active, window, cx);
            if stats.borrow().enabled {
                let cursor = frame.cursor.as_ref().map(|c| (c.line, c.col));
                let text =
                    stats.borrow().wants_text() && frame.rows.iter().flatten().any(|s| !s.text.trim().is_empty());
                stats.borrow_mut().painted(&id, cursor, text);
            }
            painted.borrow_mut().insert(id, p);
        },
    )
    .size_full()
}

#[allow(clippy::too_many_arguments)]
pub(super) fn paint_frame(
    f: &Frame,
    marks: &[CellMark],
    t: &Theme,
    bounds: Bounds<Pixels>,
    (cell_w, line_h): (Pixels, Pixels),
    (family, font_size): (&SharedString, f32),
    active: bool,
    window: &mut Window,
    cx: &mut App,
) {
    let at =
        |line: usize, col: usize| point(bounds.origin.x + cell_w * col as f32, bounds.origin.y + line_h * line as f32);
    for r in &f.backgrounds {
        window.paint_quad(fill(Bounds::new(at(r.line, r.col), size(cell_w * r.len as f32, line_h)), rgb(r.color)));
    }
    let span = |m: &CellMark| Bounds::new(at(m.row, m.from), size(cell_w * (m.to + 1 - m.from) as f32, line_h));
    for m in marks.iter().filter(|m| matches!(m.mark, Mark::Selection | Mark::Found | Mark::Current)) {
        window.paint_quad(fill(span(m), rgb(if m.mark == Mark::Selection { t.selection } else { t.found })));
    }
    let mark = f.cursor.as_ref().map(|c| {
        let w = cell_w * if c.wide { 2. } else { 1. };
        let origin = at(c.line, c.col);
        match c.shape {
            CursorShape::Beam => CursorMark::Bar(Bounds::new(origin, size(px(2.), line_h))),
            CursorShape::Underline => {
                CursorMark::Bar(Bounds::new(point(origin.x, origin.y + line_h - px(2.)), size(w, px(2.))))
            }
            CursorShape::Block if active => CursorMark::Block(Bounds::new(origin, size(w, line_h))),
            _ => CursorMark::Hollow(Bounds::new(origin, size(w, line_h))), // also any block in an inactive window
        }
    });
    let mut paint_text =
        |text: &str, line: usize, col: usize, color: u32, s: Option<&render::Span>, window: &mut Window| {
            let (bold, italic, underline, strike) =
                s.map_or((false, false, false, false), |s| (s.bold, s.italic, s.underline, s.strikeout));
            let run = TextRun {
                len: text.len(),
                font: terminal_font(family, bold, italic),
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
    if let Some(CursorMark::Block(b)) = mark {
        window.paint_quad(fill(b, rgb(t.cursor)));
    }
    for (line, spans) in f.rows.iter().enumerate() {
        for s in spans {
            paint_text(&s.text, line, s.col, s.fg, Some(s), window);
        }
    }
    for m in marks {
        let b = span(m);
        match m.mark {
            Mark::Current => window.paint_quad(outline(b, rgb(t.accent), gpui::BorderStyle::Solid)),
            Mark::Link => window.paint_quad(fill(
                Bounds::new(point(b.origin.x, b.origin.y + line_h - px(2.)), size(b.size.width, px(1.))),
                rgb(t.link),
            )),
            _ => {}
        }
    }
    match (mark, &f.cursor) {
        (Some(CursorMark::Block(_)), Some(c)) if !c.text.trim().is_empty() => {
            paint_text(&c.text, c.line, c.col, t.bg, None, window)
        }
        (Some(CursorMark::Bar(b)), _) => window.paint_quad(fill(b, rgb(t.cursor))),
        (Some(CursorMark::Hollow(b)), _) => window.paint_quad(outline(b, rgb(t.cursor), gpui::BorderStyle::Solid)),
        _ => {}
    }
}

/// How the cursor is drawn.
enum CursorMark {
    /// Filled, under the text.
    Block(Bounds<Pixels>),
    /// A beam or underline bar, over the text.
    Bar(Bounds<Pixels>),
    /// An outline: a hollow-block cursor, or the cursor of an inactive window.
    Hollow(Bounds<Pixels>),
}

impl Workspace {
    /// What pane `id` marks on the rows it shows: its selection, the find matches, the link under
    /// the mouse.
    pub(super) fn pane_marks(&self, id: &str, c: &crate::scrollback::Composed) -> Vec<CellMark> {
        use crate::scrollback::covered;
        let (cols, rows) = (c.frame.cols, c.frame.lines);
        let mut out = Vec::new();
        let mut add = |start, end, block, mark| {
            for row in 0..rows {
                if let Some((from, to)) = covered(start, end, block, c.top + row, cols) {
                    out.push(CellMark { row, from, to, mark });
                }
            }
        };
        if let Some(f) = self.term.find.as_ref().filter(|f| f.block == id) {
            let shown = c.top..c.top + rows;
            for (i, m) in
                f.matches.iter().enumerate().filter(|(_, m)| m.start.line < shown.end && m.end.line >= shown.start)
            {
                add(m.start, m.end, false, Mark::Found);
                if f.current == Some(i) {
                    add(m.start, m.end, false, Mark::Current);
                }
            }
        }
        let pv = self.term.panes.get(id);
        if let Some(s) = pv.and_then(|p| p.selection.as_ref()) {
            if let Some(r) = s.range {
                add(r.start, r.end, s.kind == leon_core::search::Kind::Block, Mark::Selection);
            }
        }
        if let Some((_, l)) = self.term.hovered.as_ref().filter(|(b, _)| b == id) {
            add(l.start, l.end, false, Mark::Link);
        }
        out.extend(self.mode_marks(id, c));
        out
    }
}
