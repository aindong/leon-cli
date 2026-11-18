// One terminal's screen, from block.watch's lines: spans and background runs placed at their
// cells (docs/architecture/protocol-and-clients.md, "Screen transport"). The server resolved every
// colour already. Text goes in with textContent only: a screen is a program's output.

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

/** Forget the measured cell width (the font changed). */
export function fontChanged() {
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
  }

  /** Apply one screen update: a whole screen when `full` or the size changed, else some rows. */
  apply(u) {
    if (u.full || u.lines !== this.lines || u.cols !== this.cols) {
      this.lines = u.lines;
      this.cols = u.cols;
      this.rows = Array.from({ length: u.lines }, () => {
        const r = document.createElement("div");
        r.className = "row";
        return r;
      });
      this.el.replaceChildren(...this.rows, this.cursor);
      this.fit();
    }
    for (const row of u.rows) if (row.line < this.lines) this.draw(this.rows[row.line], row);
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

  /** Scale the font so the whole screen fits the box (view only: the terminal keeps its size). */
  fit() {
    const box = this.el.parentElement;
    if (!box || !this.cols || !this.lines) return;
    const r = cellRatio();
    const size = Math.max(4, Math.min(box.clientWidth / (this.cols * r), box.clientHeight / (this.lines * LINE)));
    this.el.style.setProperty("--fs", `${size}px`);
    this.el.style.setProperty("--cw", `${size * r}px`);
    this.el.style.setProperty("--lh", `${size * LINE}px`);
  }
}
