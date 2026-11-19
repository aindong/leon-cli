// leon in the browser (docs/web.md): the sessions, the chosen session's tabs and the chosen tab's
// panes, live, driven like the app: keys, paste, copy and the mouse (input.js), new and closed
// sessions, tabs and panes, and Activity (activity.js). It is one more client of the socket API: a
// copy of the model refetched after each batch of events, block.watch screens for the panes shown,
// and the panes' terminals sized to the page's boxes.

import { Conn } from "./conn.js";
import { Grid, setFont } from "./grid.js";
import { wire } from "./input.js";
import { Activity } from "./activity.js";

const $ = (id) => document.getElementById(id);
const token = document.querySelector('meta[name="leon-token"]').content;

const model = { sessions: [], view: null, blocks: new Map() };
const chosen = { session: null, window: null };
const panes = new Map(); // block id → { el, grid, title, body, unwire, asked }
let watched = "";
let refetch = null;
let burst = null; // when the events waiting for a refetch began
let shown = null; // the window drawn now
let activityAt = 0;

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

function failed(e) {
  note(`${e.message || e}`);
}

function note(text) {
  $("note").textContent = text;
  $("note").hidden = !text;
}

const call = (method, params) => conn.call(method, params).catch(failed);

/** The pane that gets the keys: the shown tab's focused one. */
const target = () => (chosen.session && shown?.focused_block_id ? { s: chosen.session, b: shown.focused_block_id } : null);
const pointer = wire(conn, $("keys"), target, failed);
const activity = new Activity(conn, $("activity"), $("notify"), (e) => {
  choose(e.s, e.w);
  call("session.focus_block", { session_id: e.s, block_id: e.b });
  call("block.event", { session_id: e.s, block_id: e.b, event: { type: "focus" } }); // clears done/error
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
  setFont(settings.font.size);
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
  if (Date.now() - activityAt > 1000) {
    activityAt = Date.now();
    activity.refresh(model.sessions).catch(failed);
  }
}

function choose(session, window) {
  chosen.session = session;
  chosen.window = window;
  history.replaceState(null, "", `#${encodeURIComponent(session)}${window ? "/" + encodeURIComponent(window) : ""}`);
  refresh().catch(failed);
}

function render() {
  const win = model.view?.windows.find((w) => w.window_id === chosen.window) ?? model.view?.windows.find((w) => w.active);
  shown = win ?? null;
  renderSidebar(win);
  renderPanes(win);
  if (!model.sessions.length) note("No sessions. + Session starts one.");
}

function button(text, current, onclick, title) {
  const b = document.createElement("button");
  b.textContent = text;
  b.onclick = onclick;
  if (current) b.setAttribute("aria-current", "true");
  if (title) b.title = b.ariaLabel = title;
  return b;
}

/** A × that closes on a second click within 3 s (the first one asks). */
function closer(what, close) {
  const b = button("×", false, () => {
    if (b.dataset.armed) return close();
    b.dataset.armed = "1";
    b.textContent = "close?";
    setTimeout(() => {
      delete b.dataset.armed;
      b.textContent = "×";
    }, 3000);
  }, `Close ${what}`);
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
    li.append(row(b, closer(`session ${name}`, () => call("session.destroy", { session_id: s.session_id }))));
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
        t.append(row(button(tabName(w), w === win, open), closer(`tab ${tabName(w)}`, close)));
        tabs.append(t);
      }
      const add = document.createElement("li");
      add.append(button("+ Tab", false, () => call("session.new_window", { session_id: s.session_id }), "New tab"));
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
  call("session.new_split", { session_id: s, anchor_id: id, direction, side: "after", layout: { block: {} }, focus: true });
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
      closer("pane", () => call("block.close", { session_id: s, block_id: id })),
    );
    head.append(title, tools);
    const body = document.createElement("div");
    body.className = "body";
    const grid = new Grid();
    body.append(grid.el);
    el.append(head, body);
    const focus = () => {
      if (target()?.b !== id) call("session.focus_block", { session_id: s, block_id: id });
    };
    p = { el, grid, title, body, unwire: pointer(body, s, id, grid, focus), asked: "" };
    new ResizeObserver(() => size(id)).observe(body);
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
  const s = chosen.session;
  call("block.method", { session_id: s, block_id: id, method: "dev.leon.terminal.resize", args: { rows: fit.rows, cols: fit.cols } });
}

/** A layout node as nested boxes: `horizontal` splits side by side, `ratio` the first side's share. */
function tree(node, focused) {
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
  if (win?.zoomed_block_id) {
    out.push(tree({ block: { block_id: win.zoomed_block_id } }, win.focused_block_id));
  } else if (win?.layout) {
    out.push(tree(win.layout, win.focused_block_id));
  }
  for (const f of win?.floating ?? []) {
    if (f.hidden || win.zoomed_block_id) continue;
    const el = tree({ block: { block_id: f.block_id } }, win.focused_block_id);
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

$("new-session").onclick = () =>
  conn.call("session.create", {}).then((r) => choose(r.session_id, r.window_id), failed);
$("panes").addEventListener("mousedown", () => $("keys").focus({ preventScroll: true }));

window.addEventListener("hashchange", () => {
  fromHash();
  refresh().catch(failed);
});

start().catch(failed);
