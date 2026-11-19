// What a person does on the page, sent to the focused pane through the socket API: key presses as
// binding names (keys.js), composed text as dev.leon.terminal.write, pastes as block.event paste,
// and the mouse as block.event mouse/wheel when the program asked for it, else a selection
// (dev.leon.terminal.selection) that the copy event puts on the clipboard. Nothing here sends
// input on its own: every call follows a person's key, click or paste.

import { keyName, MAC } from "./keys.js";

/**
 * Wire the keyboard to `textarea` (kept focused while a pane is), and return `pane(…)`, which wires
 * one pane's mouse. `target()` names the pane that gets the keys: {s, b} or null.
 */
export function wire(conn, textarea, target, failed) {
  const send = (method, params) => conn.call(method, params).catch(failed);
  const event = (t, ev) => t && send("block.event", { session_id: t.s, block_id: t.b, event: ev });
  const write = (data) => {
    const t = target();
    if (t && data) send("block.method", { session_id: t.s, block_id: t.b, method: "dev.leon.terminal.write", args: { data } });
  };
  let copied = ""; // the selection's text, for the next copy

  textarea.addEventListener("keydown", (e) => {
    if (!MAC && e.ctrlKey && e.shiftKey && e.code === "KeyC") {
      document.execCommand("copy"); // the copy event below; a key press may copy without the clipboard API
      return e.preventDefault();
    }
    const name = keyName(e);
    if (!name) return; // text comes as input; ⌘ and the rest stay the browser's
    e.preventDefault();
    event(target(), { type: "key", key: name });
  });
  // text the keyboard composed; whichever of these comes second finds the field empty
  textarea.addEventListener("input", (e) => {
    if (e.isComposing) return;
    write(textarea.value);
    textarea.value = "";
  });
  textarea.addEventListener("compositionend", () => {
    write(textarea.value);
    textarea.value = "";
  });
  textarea.addEventListener("paste", (e) => {
    const data = e.clipboardData?.getData("text/plain");
    e.preventDefault();
    if (data) event(target(), { type: "paste", data });
  });
  document.addEventListener("copy", (e) => {
    if (!copied) return;
    e.clipboardData.setData("text/plain", copied);
    e.preventDefault();
  });

  const BUTTONS = ["left", "middle", "right"];
  const mods = (e) => ({ shift: e.shiftKey, alt: e.altKey, ctrl: e.ctrlKey });

  /** Wire one pane: `focus()` makes it the one that gets the keys. */
  return function pane(el, s, b, grid, focus) {
    let drag = null; // "report" while a button reports to the program, "select" while selecting
    let sel = null;
    let asking = false;
    const at = (e) => grid.cellAt(e.clientX, e.clientY);
    const report = (action, e, button) => {
      const c = at(e);
      event({ s, b }, { type: "mouse", action, button, line: c.line, col: c.col, mods: mods(e) });
    };
    const reach = async (e, text) => {
      if (!sel || (asking && !text)) return; // a drag step while one is asked is skipped, not queued
      const c = at(e);
      sel.head = { line: c.abs, col: c.col };
      asking = true;
      try {
        const r = await conn.call("block.method", {
          session_id: s,
          block_id: b,
          method: "dev.leon.terminal.selection",
          args: { kind: sel.kind, anchor: sel.anchor, head: sel.head, text },
        });
        grid.mark(r);
        if (text) copied = r?.text ?? "";
      } catch (err) {
        failed(err);
      } finally {
        asking = false;
      }
    };
    el.addEventListener("mousedown", (e) => {
      focus();
      textarea.focus({ preventScroll: true });
      e.preventDefault(); // no browser text selection, and the textarea keeps the focus
      if (grid.mouse && !e.shiftKey) {
        drag = "report";
        return report("press", e, BUTTONS[e.button] ?? "left");
      }
      if (e.button !== 0) return;
      const c = at(e);
      const kind = e.detail >= 3 ? "line" : e.detail === 2 ? "word" : "simple";
      sel = { kind, anchor: { line: c.abs, col: c.col }, head: { line: c.abs, col: c.col } };
      drag = "select";
      copied = "";
      grid.mark(null);
      if (kind !== "simple") reach(e, true);
    });
    const move = (e) => {
      if (drag === "select") return reach(e, false);
      const all = grid.mouse === "motion";
      if (drag === "report" && (grid.mouse === "drag" || all)) return report("move", e, BUTTONS[e.button] ?? "left");
      if (!drag && all && e.target.closest?.(".pane") === el) report("move", e, null);
    };
    const up = (e) => {
      if (drag === "report") report("release", e, BUTTONS[e.button] ?? "left");
      if (drag === "select" && sel) {
        const c = at(e);
        const moved = c.abs !== sel.anchor.line || c.col !== sel.anchor.col;
        if (moved || sel.kind !== "simple") reach(e, true);
        else (sel = null), grid.mark(null);
      }
      drag = null;
    };
    window.addEventListener("mousemove", move);
    window.addEventListener("mouseup", up);
    el.addEventListener("wheel", (e) => {
      if (!(grid.mouse || (grid.altScreen && grid.alternateScroll))) return; // no scrollback here yet
      e.preventDefault();
      const notches = Math.max(1, Math.round(Math.abs(e.deltaY) / 40));
      const c = at(e);
      const ev = { type: "wheel", lines: e.deltaY < 0 ? notches : -notches, line: c.line, col: c.col, mods: mods(e) };
      event({ s, b }, ev);
    }, { passive: false });
    return () => {
      window.removeEventListener("mousemove", move);
      window.removeEventListener("mouseup", up);
    };
  };
}
