// leon in the browser, view only (docs/cli/web.md): the sessions, the chosen session's tabs, and
// the chosen tab's panes, live. It is one more client of the socket API, like the app: a copy of
// the model refetched after each batch of events, and block.watch screens for the panes shown.

import { Conn } from "./conn.js";
import { Grid, fontChanged } from "./grid.js";

const $ = (id) => document.getElementById(id);
const token = document.querySelector('meta[name="leon-token"]').content;

const model = { sessions: [], view: null, blocks: new Map() };
const chosen = { session: null, window: null };
const panes = new Map(); // block id → { el, grid, title }
let watched = "";
let refetch = null;

const conn = new Conn(`ws://${location.host}/ws?token=${encodeURIComponent(token)}`, {
  onEvent: () => {
    clearTimeout(refetch);
    refetch = setTimeout(refresh, 50); // one refetch per burst of events
  },
  onScreen: (s) => panes.get(s.block_id)?.grid.apply(s),
  onClose: () => {
    note("Disconnected from the server. Reconnecting…");
    setTimeout(() => location.reload(), 3000); // a new page: a server restarted meanwhile has a new token
  },
});

function note(text) {
  $("note").textContent = text;
  $("note").hidden = !text;
}

function fromHash() {
  const [session, window] = decodeURIComponent(location.hash.slice(1)).split("/");
  chosen.session = session || null;
  chosen.window = window || null;
}

async function start() {
  fromHash();
  await conn.ready;
  const { settings } = await conn.call("server.settings");
  const family = settings.font.family.replace(/["\\]/g, "");
  document.documentElement.style.setProperty("--font", `"${family}", ui-monospace, Menlo, monospace`);
  fontChanged();
  await conn.call("session.attach_all", { event_scope: "all" });
  await refresh();
  new ResizeObserver(() => panes.forEach((p) => p.grid.fit())).observe($("panes"));
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
}

function choose(session, window) {
  chosen.session = session;
  chosen.window = window;
  history.replaceState(null, "", `#${encodeURIComponent(session)}${window ? "/" + encodeURIComponent(window) : ""}`);
  refresh();
}

function render() {
  const win = model.view?.windows.find((w) => w.window_id === chosen.window) ?? model.view?.windows.find((w) => w.active);
  renderSidebar(win);
  renderPanes(win);
  note(model.sessions.length ? "" : "No sessions. Start one with leon new on the server's machine.");
}

function renderSidebar(win) {
  const items = model.sessions.map((s, i) => {
    const li = document.createElement("li");
    const b = button(s.label || `session ${i + 1}`, s.session_id === chosen.session, () => choose(s.session_id, null));
    b.classList.add("session");
    li.append(b);
    if (s.session_id === chosen.session && model.view) {
      const tabs = document.createElement("ul");
      tabs.className = "tabs";
      for (const w of model.view.windows) {
        const t = document.createElement("li");
        t.append(button(tabName(w), w === win, () => choose(s.session_id, w.window_id)));
        tabs.append(t);
      }
      li.append(tabs);
    }
    return li;
  });
  $("sessions").replaceChildren(...items);
}

function button(text, current, onclick) {
  const b = document.createElement("button");
  b.textContent = text;
  b.onclick = onclick;
  if (current) b.setAttribute("aria-current", "true");
  return b;
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

function pane(id) {
  let p = panes.get(id);
  if (!p) {
    const el = document.createElement("section");
    el.className = "pane";
    const title = document.createElement("header");
    const body = document.createElement("div");
    body.className = "body";
    const grid = new Grid();
    body.append(grid.el);
    el.append(title, body);
    p = { el, grid, title };
    panes.set(id, p);
  }
  p.title.textContent = paneName(model.blocks.get(id));
  return p;
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
  const shown = [];
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
    el.style.flex = "";
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
    if (p.el.isConnected) shown.push(id);
    else panes.delete(id); // a new grid, and a whole screen, if it comes back
  }
  panes.forEach((p) => p.grid.fit());
  const key = shown.sort().join(" ");
  if (key !== watched) {
    watched = key;
    conn.call("block.watch", { block_ids: shown });
  }
}

window.addEventListener("hashchange", () => {
  fromHash();
  refresh();
});

start().catch((e) => note(`Can't reach the server: ${e.message || e}`));
