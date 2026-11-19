// One WebSocket carrying leon's socket API (docs/api.md): each message out is one request line,
// each message in is one line the socket would send: a reply, an event or a screen.

export class Conn {
  constructor(url, { onEvent, onScreen, onClose }) {
    this.next = 1;
    this.waiting = new Map();
    this.ws = new WebSocket(url);
    this.ready = new Promise((resolve, reject) => {
      this.ws.onopen = resolve;
      this.ws.onerror = reject;
    });
    this.ws.onclose = () => {
      for (const { reject } of this.waiting.values()) reject(new Error("connection closed"));
      this.waiting.clear();
      onClose();
    };
    this.ws.onmessage = (m) => {
      const v = JSON.parse(m.data);
      if (v.screen) return onScreen(v.screen);
      if (v.event) return onEvent(v.event);
      const w = this.waiting.get(v.id);
      if (!w) return console.warn("leon:", v.error || v);
      this.waiting.delete(v.id);
      if (v.error) w.reject(Object.assign(new Error(v.error.message), v.error));
      else w.resolve(v.result);
    };
  }

  call(method, params = {}) {
    const id = this.next++;
    return new Promise((resolve, reject) => {
      this.waiting.set(id, { resolve, reject });
      this.ws.send(JSON.stringify({ id, method, params }));
    });
  }
}
