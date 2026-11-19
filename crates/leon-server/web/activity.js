// Activity: the blocks, in every session, whose program reported working, blocked, done or error
// (OSC 7501 or `leon notify`, read with dev.leon.terminal.program_status), ordered by when the page
// first saw each state, as the app's Activity is. With the browser's permission, and only on an
// https page, a hidden page shows a notification when a block becomes blocked, done or error.

const SHOWN = ["blocked", "error", "done", "working"];
const NOTIFY = new Set(["blocked", "done", "error"]);

export class Activity {
  constructor(conn, list, button, go) {
    this.conn = conn;
    this.list = list;
    this.go = go; // go(entry): show its session and tab, focus its block
    this.seen = new Map(); // "block state" → when the page first saw it
    this.loaded = false; // nothing notifies for the states already there when the page opened
    this.button = button;
    const can = window.isSecureContext && "Notification" in window;
    button.hidden = !can || Notification.permission !== "default";
    button.onclick = async () => {
      await Notification.requestPermission();
      button.hidden = Notification.permission !== "default";
    };
  }

  /** Read every session's blocks and their states, and draw the list. */
  async refresh(sessions) {
    const entries = [];
    for (const s of sessions) {
      const [{ blocks }, view] = await Promise.all([
        this.conn.call("session.list_blocks", { session_id: s.session_id }),
        this.conn.call("session.view", { session_id: s.session_id }),
      ]);
      const tab = new Map(view.windows.map((w) => [w.window_id, w]));
      const states = await Promise.all(
        blocks.map((b) =>
          this.conn
            .call("block.method", { session_id: s.session_id, block_id: b.block_id, method: "dev.leon.terminal.program_status" })
            .then((r) => r.records.find((x) => x.id === null)?.record ?? null)
            .catch(() => null),
        ),
      );
      blocks.forEach((b, i) => {
        const rec = states[i];
        if (!rec || !SHOWN.includes(rec.state)) return;
        const w = tab.get(b.window_id);
        entries.push({ s: s.session_id, w: b.window_id, b: b.block_id, state: rec.state, msg: rec.msg ?? rec.title ?? "",
          name: b.label || b.title || b.foreground || "a pane", where: `${s.label || "session"} · ${w?.label || `tab ${w?.position ?? ""}`}` });
      });
    }
    const now = Date.now();
    for (const e of entries) {
      const key = `${e.b} ${e.state}`;
      if (this.seen.has(key)) continue;
      this.seen.set(key, now);
      if (this.loaded) this.notify(e);
    }
    for (const key of this.seen.keys()) if (!entries.some((e) => `${e.b} ${e.state}` === key)) this.seen.delete(key);
    this.loaded = true;
    entries.sort((a, b) => this.seen.get(`${b.b} ${b.state}`) - this.seen.get(`${a.b} ${a.state}`));
    this.draw(entries);
  }

  draw(entries) {
    const items = entries.map((e) => {
      const li = document.createElement("li");
      const b = document.createElement("button");
      b.className = `entry ${e.state}`;
      const dot = document.createElement("span");
      dot.className = "dot";
      const name = document.createElement("span");
      name.className = "name";
      name.textContent = `${e.name} — ${e.state}`;
      const where = document.createElement("span");
      where.className = "where";
      where.textContent = e.msg ? `${e.where}: ${e.msg}` : e.where;
      b.append(dot, name, where);
      b.onclick = () => this.go(e);
      li.append(b);
      return li;
    });
    this.list.replaceChildren(...items);
    this.list.parentElement.hidden = !items.length;
  }

  notify(e) {
    if (!NOTIFY.has(e.state) || !document.hidden || !window.isSecureContext) return;
    if (!("Notification" in window) || Notification.permission !== "granted") return;
    const n = new Notification(`${e.name} is ${e.state}`, { body: e.msg || e.where, tag: e.b });
    n.onclick = () => {
      window.focus();
      this.go(e);
      n.close();
    };
  }
}
