// One terminal's screen, from block.watch's lines: spans and background runs placed at their
// cells (docs/architecture/protocol-and-clients.md, "Screen transport"). The server resolved every
// colour already. Text goes in with textContent only: a screen is a program's output. The page
// draws at its font size and sizes the terminal to its box (app.js); a screen bigger than the box
// (another client, such as the app, set it) is drawn smaller to fit, never cut off.

const LINE = 1.25; // line height, in font sizes
let ratio = 0; // a cell's width, in font sizes, for the page's monospace font

function cellRatio() {
  if (ratio) return ratio;
  const probe = document.createElement("span");
  probe.className = "probe";
  probe.textContent = "M".repeat(50);
  document.body.append(probe);
  ratio = probe.getBoundingClientRect().width / 50 / 100 || 0.6;
  probe.remove();
  return ratio;
}

let size = 13; // the page's font size, px

/** Use `px` as the font size (server.settings' font.size), and measure the font again. */
export function setFont(px) {
  size = px;
  ratio = 0;
}

const hex = (c) => "#" + c.toString(16).padStart(6, "0");
const at = (cells) => `calc(var(--cw) * ${cells})`;

export class Grid {
  constructor() {
    this.el = document.createElement("div");
    this.el.className = "grid";
    this.lines = 0;
    this.cols = 0;
    this.rows = [];
    this.cursor = document.createElement("div");
    this.cursor.className = "cursor";
    this.marks = document.createElement("div"); // the selection's highlight
    this.marks.className = "marks";
    /** The absolute number of the screen's top line, and the mouse events the program asked for. */
    this.scrolled = 0;
    this.mouse = null;
    this.style();
  }

  /** The box around the grid, inside its padding, in px; null before it is on the page. */
  box() {
    const box = this.el.parentElement;
    if (!box) return null;
    const pad = getComputedStyle(box);
    const w = box.clientWidth - parseFloat(pad.paddingLeft) - parseFloat(pad.paddingRight);
    const h = box.clientHeight - parseFloat(pad.paddingTop) - parseFloat(pad.paddingBottom);
    return { w, h };
  }

  /** Draw at the page's font size, or smaller when the screen is bigger than the box. */
  style() {
    const r = cellRatio();
    const b = this.box();
    let px = size;
    if (b && this.cols && this.lines) px = Math.max(1, Math.min(size, b.w / (this.cols * r), b.h / (this.lines * LINE)));
    this.px = px;
    this.el.style.setProperty("--fs", `${px}px`);
    this.el.style.setProperty("--cw", `${px * r}px`);
    this.el.style.setProperty("--lh", `${px * LINE}px`);
  }

  /** The columns and rows the box fits at the page's font size. */
  fits() {
    const b = this.box();
    if (!b) return null;
    const cols = Math.floor(b.w / (size * cellRatio()));
    const rows = Math.floor(b.h / (size * LINE));
    return cols >= 2 && rows >= 1 ? { cols, rows } : null;
  }

  /** The screen cell under a point (clamped to the screen), with its absolute line. */
  cellAt(x, y) {
    const r = this.el.getBoundingClientRect();
    const px = this.px || size;
    const col = Math.max(0, Math.min(this.cols - 1, Math.floor((x - r.left) / (px * cellRatio()))));
    const line = Math.max(0, Math.min(this.lines - 1, Math.floor((y - r.top) / (px * LINE))));
    return { line, col, abs: this.scrolled + line };
  }

  /** Highlight a selection (absolute lines, both ends inclusive), or none. */
  mark(sel) {
    this.sel = sel;
    const parts = [];
    for (let abs = sel?.start.line ?? 1; sel && abs <= sel.end.line; abs++) {
      const line = abs - this.scrolled;
      if (line < 0 || line >= this.lines) continue;
      const from = abs === sel.start.line ? sel.start.col : 0;
      const to = abs === sel.end.line ? sel.end.col + 1 : this.cols;
      const d = document.createElement("span");
      d.style.left = at(from);
      d.style.width = at(Math.max(0, to - from));
      d.style.top = `calc(var(--lh) * ${line})`;
      parts.push(d);
    }
    this.marks.replaceChildren(...parts);
  }

  /** Apply one screen update: a whole screen when `full` or the size changed, else some rows. */
  apply(u) {
    this.scrolled = u.scrolled ?? 0;
    this.mouse = u.mouse ?? null;
    this.altScreen = !!u.alt_screen;
    this.alternateScroll = !!u.alternate_scroll;
    if (u.full || u.lines !== this.lines || u.cols !== this.cols) {
      this.lines = u.lines;
      this.cols = u.cols;
      this.rows = Array.from({ length: u.lines }, () => {
        const r = document.createElement("div");
        r.className = "row";
        return r;
      });
      this.el.replaceChildren(...this.rows, this.marks, this.cursor);
      this.style();
    }
    for (const row of u.rows) if (row.line < this.lines) this.draw(this.rows[row.line], row);
    if (this.sel && u.scrolled !== undefined) this.mark(this.sel); // it is on absolute lines
    this.place(u.cursor);
  }

  draw(el, row) {
    const parts = [];
    for (const b of row.backgrounds) {
      const d = document.createElement("span");
      d.className = "bg";
      d.style.left = at(b.col);
      d.style.width = at(b.len);
      d.style.background = hex(b.color);
      parts.push(d);
    }
    for (const s of row.spans) {
      const t = document.createElement("span");
      t.className = "fg";
      t.textContent = s.text;
      t.style.left = at(s.col);
      t.style.color = hex(s.fg);
      if (s.bold) t.style.fontWeight = "700";
      if (s.italic) t.style.fontStyle = "italic";
      const lines = [s.underline && "underline", s.strikeout && "line-through"].filter(Boolean);
      if (lines.length) t.style.textDecoration = lines.join(" ");
      parts.push(t);
    }
    el.replaceChildren(...parts);
  }

  place(c) {
    const shape = c ? c.shape : "hidden";
    this.cursor.dataset.shape = shape;
    if (shape === "hidden") return;
    this.cursor.style.left = at(c.col);
    this.cursor.style.top = `calc(var(--lh) * ${c.line})`;
    this.cursor.style.width = at(c.wide ? 2 : 1);
    this.cursor.textContent = shape === "block" ? c.text : "";
  }
}
