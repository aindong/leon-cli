// Browser keys to leon's binding names (`enter`, `ctrl+c`, `alt+left`): the page sends the name in
// `block.event {type: "key"}`, and the server makes the bytes with the same mapping the app and
// `leon attach` use (leon_core::keys::encode), in the program's cursor-key mode at that moment.
// Text the keyboard composes (letters, accents, IME) isn't a key here: the page types it as text.
// NAMES stays JSON-shaped: crates/leon/tests/web_keys_table.rs reads it.

export const NAMES = {
  "Enter": "enter",
  "Escape": "escape",
  "Tab": "tab",
  "Backspace": "backspace",
  "ArrowUp": "up",
  "ArrowDown": "down",
  "ArrowLeft": "left",
  "ArrowRight": "right",
  "Home": "home",
  "End": "end",
  "PageUp": "pageup",
  "PageDown": "pagedown",
  "Insert": "insert",
  "Delete": "delete",
  "F1": "f1",
  "F2": "f2",
  "F3": "f3",
  "F4": "f4",
  "F5": "f5",
  "F6": "f6",
  "F7": "f7",
  "F8": "f8",
  "F9": "f9",
  "F10": "f10",
  "F11": "f11",
  "F12": "f12",
  " ": "space"
};

/** The character a key types without modifiers, from its place on the keyboard (`KeyA` → `a`). */
const PLACES = {
  Minus: "-", Equal: "=", BracketLeft: "[", BracketRight: "]", Backslash: "\\", Semicolon: ";",
  Quote: "'", Comma: ",", Period: ".", Slash: "/", Backquote: "`",
};

function place(code) {
  if (/^Key[A-Z]$/.test(code)) return code.slice(3).toLowerCase();
  if (/^Digit[0-9]$/.test(code)) return code.slice(5);
  return PLACES[code] ?? null;
}

/** The characters ctrl makes a control code of (leon_core::keys's `control`); JSON-shaped. */
export const CTRL = "abcdefghijklmnopqrstuvwxyz@2[\\]^6_-/?";

export const MAC = /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent);

/**
 * The binding name for a key press, or null when the page shouldn't send one: a lone modifier,
 * text (typed through the textarea instead), ⌘ (the browser's), AltGr text, and outside macOS
 * ctrl+v (the browser's paste) and ctrl+shift+c (the page's copy). As in the app, a Mac's Option
 * with a character is the text it composes (`ß`); elsewhere Alt with a character is Meta. Ctrl
 * takes the key's place on the keyboard, so ctrl+c is ctrl+c on any layout.
 */
export function keyName(e) {
  if (e.metaKey || e.isComposing || e.keyCode === 229 || e.getModifierState?.("AltGraph")) return null;
  // outside macOS, shift+Insert pastes and ctrl+Insert copies, as in other terminals: the browser's
  if (!MAC && e.key === "Insert" && (e.shiftKey || e.ctrlKey)) return null;
  const named = NAMES[e.key];
  const mods = (shift) => (e.ctrlKey ? "ctrl+" : "") + (e.altKey ? "alt+" : "") + (shift && e.shiftKey ? "shift+" : "");
  if (named) return mods(true) + named;
  if (!e.ctrlKey && !(e.altKey && !MAC)) return null; // text (Option's too), or a lone modifier
  // ctrl: the layout's own Latin letter (Dvorak's C is ctrl+c), else the key's place (other scripts)
  const latin = /^[a-z]$/i.test(e.key) ? e.key.toLowerCase() : null;
  const c = e.ctrlKey ? latin ?? place(e.code) : e.key.length === 1 ? e.key.toLowerCase() : null;
  if (!c || c.length !== 1 || c === "+" || (e.ctrlKey && !CTRL.includes(c))) return null;
  if (!MAC && e.ctrlKey && !e.altKey && (c === "v" || (c === "c" && e.shiftKey))) return null;
  const shift = !e.ctrlKey && /^[A-Z]$/.test(e.key) ? "shift+" : ""; // alt+B is ESC B, not ESC b
  return mods(false) + shift + c;
}
