// leon in the browser on a phone (docs/web.md#on-a-phone): when the page is narrow it shows Home
// (Activity, then the sessions) or one pane, with a strip above it (‹ Home, ☰ the drawer, the
// tab's other panes) and a row of the keys a phone's keyboard lacks. app.js decides what is shown;
// this builds the row.

import { ROW } from "./keys.js";

/** When the page is a phone's: the layout and the font size follow it. */
export const narrow = matchMedia("(max-width: 700px)");

/** A phone's font size: the server's, kept between 11 and 14 px, so ~40 columns fit a portrait phone. */
export const phoneFont = (px) => Math.min(14, Math.max(11, px));

/**
 * Fill `el` with the key row (keys.js's ROW): a tap sends `press(entry)` and leaves the focus on
 * the field, so the keyboard stays up. Returns `ctrl(on)`, which shows the sticky Ctrl.
 */
export function keyRow(el, press) {
  const buttons = ROW.map((entry) => {
    const b = document.createElement("button");
    b.textContent = entry.label;
    if (entry.ctrl) b.setAttribute("aria-pressed", "false");
    b.addEventListener("mousedown", (e) => e.preventDefault());
    b.onclick = () => press(entry);
    return b;
  });
  el.replaceChildren(...buttons);
  const ctrl = buttons[ROW.findIndex((e) => e.ctrl)];
  return (on) => ctrl.setAttribute("aria-pressed", String(on));
}
