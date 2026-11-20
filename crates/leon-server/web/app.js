// leon in the browser (docs/web.md): the sessions, the chosen session's tabs and the chosen tab's
// panes, live, driven like the app: keys, paste, copy and the mouse (input.js), new and closed
// sessions, tabs and panes, and Activity (activity.js). It is one more client of the socket API: a
// copy of the model refetched after each batch of events, block.watch screens for the panes shown,
// and the panes' terminals sized to the page's boxes.

import { Conn } from "./conn.js";
import { Grid, setFont } from "./grid.js";
import { wire } from "./input.js";
import { Activity } from "./activity.js";
import { keyRow, narrow, phoneFont } from "./phone.js";

const $ = (id) => document.getElementById(id);
const token = document.querySelector('meta[name="leon-token"]').content;

const model = { sessions: [], view: null, blocks: new Map() };
const chosen = { session: null, window: null };
const panes = new Map(); // block id → { el, grid, title, body, unwire, asked }
let watched = "";
let refetch = null;
let burst = null; // when the events waiting for a refetch began
let shown = null; // the window drawn now
let view = "home"; // on a phone: Home (Activity and the sessions) or the shown tab's one pane
let moves = 0; // the person's navigations: what a slow + Tab or + Session made opens only if none came since
let lastTarget = ""; // the pane the keys went to at the last render
let font = 13; // the server's font size; a phone's is kept readable (phone.js)
const phone = () => narrow.matches;

const scheme = location.protocol === "https:" ? "wss" : "ws";
const conn = new Conn(`${scheme}://${location.host}/ws?token=${encodeURIComponent(token)}`, {
  onEvent: () => {
    // one refetch per burst of events, and at least one every half second while they keep coming
    clearTimeout(refetch);
    burst ??= Date.now();
    refetch = setTimeout(() => {
      burst = null;
      refresh().catch(failed);
    }, Date.now() - burst > 500 ? 0 : 50);
  },
  onScreen: (s) => panes.get(s.block_id)?.grid.apply(s),
  onClose: () => {
    closed = true;
    note("Disconnected from the server. Reconnecting…");
    reconnect(2000);
  },
});

/** Reload once the server answers again: a new page, since serving again means a new token. */
function reconnect(wait) {
  setTimeout(async () => {
    try {
      if ((await fetch("/", { cache: "no-store" })).ok) return location.reload();
    } catch {
      // still away
    }
    reconnect(Math.min(wait * 2, 30000));
  }, wait);
}

let closed = false; // the socket closed: its pending calls' errors don't replace that note

function failed(e) {
  if (!closed) note(`${e.message || e}`);
}

function note(text) {
  $("note").textContent = text;
  $("note").hidden = !text;
}

const call = (method, params) => conn.call(method, params).catch(failed);

/**
 * The pane that gets the keys: the one the person chose on this page in the shown tab, else the
 * tab's focused one. A focus move the page didn't make (another client, or a program calling
 * session.focus_block) doesn't redirect what the person is typing.
 */
const picked = new Map(); // window id → block id the person clicked or went to here
const target = () => {
  // while another session is being fetched, `shown` is still the last one's: no target in between
  if (!chosen.session || !shown || model.view?.session_id !== chosen.session) return null;
  const mine = picked.get(shown.window_id);
  const zoomed = shown.zoomed_block_id;
  const b = mine && shown.block_ids.includes(mine) && (!zoomed || mine === zoomed) ? mine : zoomed ?? shown.focused_block_id;
  return b ? { s: chosen.session, b } : null;
};
// Shift+Escape leaves the pane for the sidebar (on a phone, the drawer), where Tab moves between the buttons
const leave = () => {
  if (phone() && view === "pane") drawer(true);
  $("sessions").querySelector("button")?.focus();
};
let showCtrl = () => {};
const input = wire(conn, $("keys"), target, failed, leave, (on) => showCtrl(on));
showCtrl = keyRow($("keyrow"), (entry) => {
  input.press(entry);
  $("keys").focus({ preventScroll: true }); // the keyboard stays up
});

/** The drawer: on a phone, the sidebar over the pane. */
function drawer(open) {
  document.body.classList.toggle("drawer", open);
  $("menu").setAttribute("aria-expanded", String(open));
}

/** The layout for the page's width: a phone's (Home or one pane) or the desktop's. */
function layout() {
  document.body.classList.toggle("phone", phone());
  document.body.dataset.view = view;
  if (!phone()) drawer(false), input.unstick(); // the row's Ctrl is gone with the row
  setFont(phone() ? phoneFont(font) : font);
}
const activity = new Activity(conn, $("activity"), $("notify"), async (e) => {
  picked.set(e.w, e.b);
  choose(e.s, e.w);
  // a tab zoomed on another pane would keep showing that one: unzoom it first
  const tabs = await conn.call("session.view", { session_id: e.s }).catch(() => null);
  const zoomed = tabs?.windows.find((w) => w.window_id === e.w)?.zoomed_block_id;
  if (zoomed && zoomed !== e.b) await call("session.zoom_block", { session_id: e.s, block_id: zoomed, zoom: false });
  call("session.focus_block", { session_id: e.s, block_id: e.b });
  call("block.event", { session_id: e.s, block_id: e.b, event: { type: "focus" } }); // clears done/error
  $("keys").focus({ preventScroll: true }); // typing goes on to the pane
});

function fromHash() {
  const decode = (part) => {
    try {
      return decodeURIComponent(part || "") || null;
    } catch {
      return null; // a hand-edited hash: choose the default
    }
  };
  const [session, window] = location.hash.slice(1).split("/");
  chosen.session = decode(session);
  chosen.window = decode(window);
}

async function start() {
  fromHash();
  await conn.ready;
  const { settings } = await conn.call("server.settings");
  const family = settings.font.family.replace(/["\\]/g, "");
  document.documentElement.style.setProperty("--font", `"${family}", ui-monospace, Menlo, monospace`);
  font = settings.font.size;
  layout();
  await conn.call("session.attach_all", { event_scope: "all" });
  await refresh();
  $("keys").focus({ preventScroll: true });
}

async function refresh() {
  model.sessions = (await conn.call("session.list")).sessions;
  if (!model.sessions.some((s) => s.session_id === chosen.session)) {
    chosen.session = model.sessions[0]?.session_id ?? null;
  }
  if (chosen.session) {
    const id = chosen.session;
    const [view, { blocks }] = await Promise.all([
      conn.call("session.view", { session_id: id }),
      conn.call("session.list_blocks", { session_id: id }),
    ]);
    if (id !== chosen.session) return; // chosen again while this was asked
    model.view = view;
    model.blocks = new Map(blocks.map((b) => [b.block_id, b]));
  } else {
    model.view = null;
  }
  render();
  note(model.sessions.length ? "" : "No sessions. + Session starts one.");
  activity.refresh(model.sessions);
}

function choose(session, window) {
  moves++;
  chosen.session = session;
  chosen.window = window;
  view = "pane"; // on a phone, what was chosen opens
  drawer(false);
  input.unstick(); // a Ctrl from the last pane doesn't carry over
  document.body.dataset.view = view;
  history.replaceState(null, "", `#${encodeURIComponent(session)}${window ? "/" + encodeURIComponent(window) : ""}`);
  refresh().catch(failed);
}

function render() {
  const win = model.view?.windows.find((w) => w.window_id === chosen.window) ?? model.view?.windows.find((w) => w.active);
  shown = win ?? null;
  // the pane the keys go to when this page first shows a tab is the person's from then on
  if (shown && !picked.has(shown.window_id)) {
    const first = shown.zoomed_block_id ?? shown.focused_block_id;
    if (first) picked.set(shown.window_id, first);
  }
  // the sticky Ctrl is for the pane it was pressed for: another pane (one closed under it) drops it
  const now = target();
  const key = now ? `${now.s} ${now.b}` : "";
  if (key !== lastTarget) (lastTarget = key), input.unstick();
  renderSidebar(win);
  renderPanes(win);
  renderStrip(win);
}

/** On a phone, the strip over the pane: its name, and the tab's other panes to switch to. */
function renderStrip(win) {
  if (!phone()) return;
  const now = target()?.b;
  $("strip-title").textContent = paneName(model.blocks.get(now)) || (win ? tabName(win) : "");
  // a zoomed tab shows only its zoomed pane: no other pane to switch to
  const others = win?.zoomed_block_id ? [] : (win?.block_ids ?? []).filter((id) => id !== now);
  $("chips").replaceChildren(
    ...others.map((id) =>
      button(paneName(model.blocks.get(id)) || "pane", false, () => {
        input.unstick();
        picked.set(win.window_id, id);
        call("session.focus_block", { session_id: chosen.session, block_id: id });
        render();
      }),
    ),
  );
}

function button(text, current, onclick, title) {
  const b = document.createElement("button");
  b.textContent = text;
  b.onclick = () => {
    onclick();
    $("keys").focus({ preventScroll: true }); // typing goes on to the pane
  };
  if (current) b.setAttribute("aria-current", "true");
  if (title) b.title = b.ariaLabel = title;
  return b;
}

const armed = new Map(); // what a × asked to close → until when, kept across re-renders

/** A × that closes `key` on a second click within 3 s (the first one asks). */
function closer(what, key, close) {
  const b = button("×", false, () => {
    if ((armed.get(key) ?? 0) > Date.now()) return armed.delete(key), close();
    armed.set(key, Date.now() + 3000);
    show();
    setTimeout(show, 3000);
  }, `Close ${what}`);
  const show = () => {
    const on = (armed.get(key) ?? 0) > Date.now();
    if (on) b.dataset.armed = "1";
    else delete b.dataset.armed;
    b.textContent = on ? "close?" : "×";
  };
  show();
  b.classList.add("close");
  return b;
}

function row(main, ...extra) {
  const r = document.createElement("div");
  r.className = "row-actions";
  r.append(main, ...extra);
  return r;
}

function renderSidebar(win) {
  const items = model.sessions.map((s, i) => {
    const li = document.createElement("li");
    const name = s.label || `session ${i + 1}`;
    const b = button(name, s.session_id === chosen.session, () => choose(s.session_id, null));
    b.classList.add("session");
    li.append(row(b, closer(`session ${name}`, s.session_id, () => call("session.destroy", { session_id: s.session_id }))));
    if (s.session_id === chosen.session && model.view) {
      const tabs = document.createElement("ul");
      tabs.className = "tabs";
      for (const w of model.view.windows) {
        const t = document.createElement("li");
        const open = () => {
          choose(s.session_id, w.window_id);
          call("session.focus_window", { session_id: s.session_id, window_id: w.window_id });
        };
        const close = () => call("session.close_window", { session_id: s.session_id, window_id: w.window_id });
        t.append(row(button(tabName(w), w === win, open), closer(`tab ${tabName(w)}`, w.window_id, close)));
        tabs.append(t);
      }
      const add = document.createElement("li");
      const tab = () => {
        const before = moves;
        // open it, unless the person went elsewhere while it was made
        call("session.new_window", { session_id: s.session_id }).then((r) => r?.window_id && moves === before && choose(s.session_id, r.window_id));
      };
      add.append(button("+ Tab", false, tab, "New tab"));
      add.firstChild.classList.add("add");
      tabs.append(add);
      li.append(tabs);
    }
    return li;
  });
  $("sessions").replaceChildren(...items);
}

function tabName(w) {
  const b = model.blocks.get(w.focused_block_id);
  return w.label || b?.title || b?.foreground || `tab ${w.position}`;
}

function paneName(b) {
  if (!b) return "";
  const where = b.cwd ? b.cwd.split("/").filter(Boolean).pop() || "/" : "";
  return b.label || b.title || [b.foreground, where].filter(Boolean).join(" · ");
}

function split(id, direction) {
  const s = chosen.session;
  const w = shown?.window_id;
  call("session.new_split", { session_id: s, anchor_id: id, direction, side: "after", layout: { block: {} }, focus: true })
    .then((r) => r?.block_id && w && picked.set(w, r.block_id)); // the new pane gets the keys, as asked
}

function pane(id) {
  let p = panes.get(id);
  if (!p) {
    const el = document.createElement("section");
    const head = document.createElement("header");
    const title = document.createElement("span");
    title.className = "title";
    const tools = document.createElement("span");
    tools.className = "tools";
    const s = chosen.session;
    tools.append(
      button("◫", false, () => split(id, "horizontal"), "Split right"),
      button("⊟", false, () => split(id, "vertical"), "Split down"),
      closer("pane", id, () => call("block.close", { session_id: s, block_id: id })),
    );
    head.append(title, tools);
    const body = document.createElement("div");
    body.className = "body";
    const grid = new Grid();
    body.append(grid.el);
    el.append(head, body);
    const focus = () => {
      if (shown) picked.set(shown.window_id, id); // at once: the next key goes here, not to the last pane
      p.el.parentElement && document.querySelectorAll(".pane.focused").forEach((x) => x.classList.toggle("focused", x === p.el));
      p.el.classList.add("focused");
      call("session.focus_block", { session_id: s, block_id: id });
    };
    const watch = new ResizeObserver(() => size(id));
    watch.observe(body);
    const unwire = input.pane(body, s, id, grid, focus);
    p = { el, grid, title, body, s, unwire: () => (unwire(), watch.disconnect()), asked: "" };
    panes.set(id, p);
  }
  p.title.textContent = paneName(model.blocks.get(id));
  p.el.className = "pane"; // a pane moves between a split, a float and a zoom: start clean
  p.el.style.cssText = "";
  return p;
}

/**
 * Size a pane's terminal to what its box fits, when the box (the page's own layout of it) changed
 * since the page last asked; never because another client resized it (`leon attach`'s rule).
 */
function size(id) {
  const p = panes.get(id);
  if (!p?.el.isConnected) return;
  p.grid.style();
  const fit = p.grid.fits();
  const key = fit ? `${fit.cols}x${fit.rows}` : "";
  if (!fit || key === p.asked) return;
  p.asked = key;
  call("block.method", { session_id: p.s, block_id: id, method: "dev.leon.terminal.resize", args: { rows: fit.rows, cols: fit.cols } });
}

/** A layout node as nested boxes: `horizontal` splits side by side, `ratio` the first side's share. */
function tree(node, focused) {
  if (node === shown?.layout || node.block?.block_id === shown?.zoomed_block_id) focused = target()?.b ?? focused;
  if (node.block) {
    const p = pane(node.block.block_id);
    p.el.classList.toggle("focused", node.block.block_id === focused);
    return p.el;
  }
  const s = node.split;
  const box = document.createElement("div");
  box.className = `split ${s.direction}`;
  const [a, b] = [tree(s.before, focused), tree(s.after, focused)];
  a.style.flex = `${s.ratio} 1 0`;
  b.style.flex = `${1 - s.ratio} 1 0`;
  box.append(a, b);
  return box;
}

function renderPanes(win) {
  const ids = [];
  const out = [];
  if (phone()) {
    // Home shows no pane; a pane view, the one the keys go to, filling the screen
    const t = view === "pane" ? target() : null;
    if (t) out.push(tree({ block: { block_id: t.b } }, t.b));
  } else if (win?.zoomed_block_id) {
    out.push(tree({ block: { block_id: win.zoomed_block_id } }, win.focused_block_id));
  } else if (win?.layout) {
    out.push(tree(win.layout, win.focused_block_id));
  }
  for (const f of win?.floating ?? []) {
    if (f.hidden || win.zoomed_block_id || phone()) continue;
    const el = tree({ block: { block_id: f.block_id } }, target()?.b ?? win.focused_block_id);
    el.classList.add("float");
    Object.assign(el.style, {
      left: `${f.bounds.x * 100}%`,
      top: `${f.bounds.y * 100}%`,
      width: `${f.bounds.width * 100}%`,
      height: `${f.bounds.height * 100}%`,
    });
    out.push(el);
  }
  $("panes").replaceChildren(...out);
  for (const [id, p] of panes) {
    if (p.el.isConnected) ids.push(id);
    else (p.unwire(), panes.delete(id)); // a new grid, and a whole screen, if it comes back
  }
  ids.forEach(size);
  const key = ids.sort().join(" ");
  if (key !== watched) {
    watched = key;
    conn.call("block.watch", { block_ids: ids }).catch(() => {
      watched = ""; // a block went away meanwhile: the next render asks again
    });
  }
}

$("new-session").onclick = () => {
  const before = moves;
  conn.call("session.create", {}).then((r) => moves === before && choose(r.session_id, r.window_id), failed);
};
$("panes").addEventListener("mousedown", () => $("keys").focus({ preventScroll: true }));
$("home").onclick = () => {
  moves++;
  view = "home";
  input.unstick();
  layout();
  render();
};
$("menu").onclick = () => drawer(!document.body.classList.contains("drawer"));
$("scrim").onclick = () => drawer(false);
narrow.addEventListener("change", () => {
  layout();
  render();
});
// the part of the page a phone's keyboard leaves visible, so the key row stays above it (iOS keeps
// the layout's height when the keyboard opens; Android follows the viewport tag)
const seen = () => document.documentElement.style.setProperty("--seen", `${visualViewport.height}px`);
if (window.visualViewport) {
  visualViewport.addEventListener("resize", seen);
  seen();
}

window.addEventListener("hashchange", () => {
  fromHash();
  refresh().catch(failed);
});

start().catch(failed);
