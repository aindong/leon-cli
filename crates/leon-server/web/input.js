// What a person does on the page, sent to the focused pane through the socket API: key presses as
// binding names (keys.js), composed text as dev.leon.terminal.write, pastes as block.event paste,
// and the mouse as block.event mouse/wheel when the program asked for it, else a selection
// (dev.leon.terminal.selection) that the copy event puts on the clipboard. Nothing here sends
// input on its own: every call follows a person's key, click or paste.

import { CTRL, ctrlChar, keyName, MAC } from "./keys.js";

/**
 * Wire the keyboard to `textarea` (kept focused while a pane is), and return `pane(…)`, which wires
 * one pane's mouse, and `press(entry)`, a key of the phone's key row (keys.js's ROW). `target()`
 * names the pane that gets the keys: {s, b} or null. Shift+Escape calls `leave()`: the way out of
 * the pane for the keyboard, since Tab goes to the program. `sticky(on)` shows the row's Ctrl.
 */
export function wire(conn, textarea, target, failed, leave, sticky) {
  const send = (method, params) => conn.call(method, params).catch(failed);
  const event = (t, ev) => t && send("block.event", { session_id: t.s, block_id: t.b, event: ev });
  const write = (data) => {
    const t = target();
    if (t && data) send("block.method", { session_id: t.s, block_id: t.b, method: "dev.leon.terminal.write", args: { data } });
  };
  // the key row's Ctrl: the next key or character, from the row or the keyboard, goes as ctrl+ it
  let ctrl = false;
  const setCtrl = (on) => ((ctrl = on), sticky?.(on));
  const key = (name) => {
    const t = target();
    if (ctrl && !name.startsWith("ctrl+")) name = `ctrl+${name}`;
    setCtrl(false);
    event(t, { type: "key", key: name });
  };
  /** Text typed or composed: one character under the sticky Ctrl is a control key, if it has one. */
  const text = (data) => {
    if (ctrl && data.length === 1 && CTRL.includes(data.toLowerCase())) return key(data.toLowerCase());
    if (data) setCtrl(false);
    write(data);
  };
  /** Send what the field still holds (a word a phone's keyboard hasn't committed), before a key. */
  const flush = () => {
    const held = textarea.value;
    textarea.value = "";
    if (held) write(held);
  };
  const press = (entry) => {
    if (entry.ctrl) return setCtrl(!ctrl);
    flush();
    entry.key ? key(entry.key) : text(entry.text);
  };
  let copied = ""; // the selection's text, for the next copy
  let owner = null; // the pane whose selection `copied` is
  let gen = 0; // bumped when any pane's selection starts or clears: a late answer is dropped
  /** The longest paste, JSON-escaped: one request line is at most 1 MiB, with room for the rest. */
  const MOST_PASTE = (1 << 20) - (4 << 10);

  textarea.addEventListener("keydown", (e) => {
    if (e.key === "Escape" && e.shiftKey && !e.ctrlKey && !e.altKey && !e.metaKey && leave) {
      e.preventDefault();
      return leave();
    }
    if (!MAC && e.ctrlKey && e.shiftKey && !e.altKey && ctrlChar(e) === "c") {
      document.execCommand("copy"); // the copy event below; a key press may copy without the clipboard API
      return e.preventDefault();
    }
    const name = keyName(e);
    if (!name) return; // text comes as input; ⌘ and the rest stay the browser's
    e.preventDefault();
    key(name);
  });
  // a phone's keyboard names no keys (keyCode 229): Backspace and Enter come as edits of the field
  textarea.addEventListener("beforeinput", (e) => {
    const k = {
      deleteContentBackward: "backspace",
      deleteWordBackward: "ctrl+w",
      insertLineBreak: "enter",
      insertParagraph: "enter",
    }[e.inputType];
    if (!k || e.isComposing) return;
    if (k !== "enter" && textarea.value) return; // the field's own text, not yet sent: let it edit that
    e.preventDefault();
    flush(); // Enter goes after the text it ends
    key(k);
  });
  // text the keyboard composed; whichever of these comes second finds the field empty
  textarea.addEventListener("input", (e) => {
    if (e.isComposing) return;
    text(textarea.value);
    textarea.value = "";
  });
  textarea.addEventListener("compositionend", () => {
    text(textarea.value);
    textarea.value = "";
  });
  textarea.addEventListener("paste", (e) => {
    const data = e.clipboardData?.getData("text/plain");
    e.preventDefault();
    if (data && new TextEncoder().encode(JSON.stringify(data)).length > MOST_PASTE) {
      return failed(new Error("That paste is too big for the page (over 1 MB once encoded)"));
    }
    setCtrl(false);
    if (data) event(target(), { type: "paste", data });
  });
  document.addEventListener("copy", (e) => {
    if (!copied || String(document.getSelection() ?? "")) return; // other text on the page: the browser's
    e.clipboardData.setData("text/plain", copied);
    e.preventDefault();
  });

  const BUTTONS = ["left", "middle", "right"];
  const mods = (e) => ({ shift: e.shiftKey, alt: e.altKey, ctrl: e.ctrlKey });

  /** Wire one pane: `focus()` makes it the one that gets the keys. */
  const pane = (el, s, b, grid, focus) => {
    let drag = null; // "report" while a button reports to the program, "select" while selecting
    let held = "left"; // the button a report drag holds
    let last = ""; // the cell last reported, so a move within it isn't sent again
    let sel = null;
    let asking = false;
    let gone = false; // the pane went: its late answer is dropped
    const me = {};
    const at = (e) => grid.cellAt(e.clientX, e.clientY);
    const report = (action, e, button) => {
      const c = at(e);
      const cell = `${c.line} ${c.col}`;
      if (action === "move" && cell === last) return;
      last = cell;
      event({ s, b }, { type: "mouse", action, button, line: c.line, col: c.col, mods: mods(e) });
    };
    const reach = async (e, text) => {
      if (!sel || (asking && !text)) return; // a drag step while one is asked is skipped, not queued
      const c = at(e);
      sel.head = { line: c.abs, col: c.col };
      asking = true;
      const g = gen;
      try {
        const r = await conn.call("block.method", {
          session_id: s,
          block_id: b,
          method: "dev.leon.terminal.selection",
          args: { kind: sel.kind, anchor: sel.anchor, head: sel.head, text },
        });
        if (g !== gen || gone) return;
        grid.mark(r);
        if (text) (copied = r?.text ?? ""), (owner = me);
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
        held = BUTTONS[e.button] ?? "left";
        return report("press", e, held);
      }
      if (e.button !== 0) return;
      const c = at(e);
      const kind = e.detail >= 3 ? "line" : e.detail === 2 ? "word" : "simple";
      sel = { kind, anchor: { line: c.abs, col: c.col }, head: { line: c.abs, col: c.col } };
      drag = "select";
      gen++;
      copied = "";
      grid.mark(null);
      if (kind !== "simple") reach(e, true);
    });
    const move = (e) => {
      if (drag === "select") return reach(e, false);
      const all = grid.mouse === "motion";
      if (drag === "report" && (grid.mouse === "drag" || all)) return report("move", e, held);
      if (!drag && all && el.contains(e.target)) report("move", e, null);
    };
    const up = (e) => {
      if (drag === "report") report("release", e, held);
      if (drag === "select" && sel) {
        const c = at(e);
        const moved = c.abs !== sel.anchor.line || c.col !== sel.anchor.col;
        if (moved || sel.kind !== "simple") reach(e, true);
        else (sel = null), gen++, grid.mark(null);
      }
      drag = null;
    };
    window.addEventListener("mousemove", move);
    window.addEventListener("mouseup", up);
    let wheel = 0; // pixels not yet a whole notch, as the app adds up a trackpad's
    el.addEventListener("wheel", (e) => {
      if (!(grid.mouse || (grid.altScreen && grid.alternateScroll))) return; // no scrollback here yet
      e.preventDefault();
      const px = e.deltaMode === 1 ? e.deltaY * 40 : e.deltaMode === 2 ? e.deltaY * 400 : e.deltaY;
      wheel += px;
      const notches = Math.trunc(wheel / 40);
      if (!notches) return;
      wheel -= notches * 40;
      const c = at(e);
      event({ s, b }, { type: "wheel", lines: -notches, line: c.line, col: c.col, mods: mods(e) });
    }, { passive: false });
    return () => {
      window.removeEventListener("mousemove", move);
      window.removeEventListener("mouseup", up);
      gone = true;
      if (owner === me) (copied = ""), (owner = null); // its pane is gone
    };
  };
  return { pane, press, unstick: () => setCtrl(false) };
}
