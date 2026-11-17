"""build.py: render services.json into site/index.html, a one-page status board."""
import html
import json
import pathlib
import time

ROOT = pathlib.Path(__file__).parent
LABEL = {"up": "Operational", "degraded": "Degraded", "down": "Outage"}

CSS = """
body { font: 15px/1.5 system-ui, sans-serif; margin: 0; background: #0f1115; color: #e6e6e6; }
main { max-width: 760px; margin: 48px auto; padding: 0 24px; }
.service { display: grid; grid-template-columns: 1fr 140px 90px 90px; padding: 14px 0;
           border-bottom: 1px solid #23262d; }
@media (max-width: 600px) {
  main { margin: 24px auto; padding: 0 16px; }
  .service { grid-template-columns: 1fr auto; row-gap: 4px; }
}
.up { color: #4ade80; } .degraded { color: #facc15; } .down { color: #f87171; }
"""


def row(s):
    latency = f"{s['latency_ms']} ms" if s["latency_ms"] is not None else "—"
    return (
        f'<div class="service"><span>{html.escape(s["name"])}</span>'
        f'<span class="{s["status"]}">{LABEL[s["status"]]}</span>'
        f"<span>{latency}</span><span>{s['uptime']:.2f}%</span></div>"
    )


def render(services):
    worst = "down" if any(s["status"] == "down" for s in services) else (
        "degraded" if any(s["status"] == "degraded" for s in services) else "up")
    head = ('<meta charset="utf-8"><meta name="viewport"'
            ' content="width=device-width, initial-scale=1">'
            '<title>Status</title>')
    return (
        f"<!doctype html><html><head>{head}<style>{CSS}</style></head><body><main>"
        f'<h1 class="{worst}">{ {"up": "All systems normal", "degraded": "Some systems degraded", "down": "Partial outage"}[worst] }</h1>'
        + "".join(row(s) for s in services)
        + "</main></body></html>"
    )


def build():
    start = time.perf_counter()
    services = json.loads((ROOT / "services.json").read_text())
    out = ROOT / "site" / "index.html"
    out.parent.mkdir(exist_ok=True)
    out.write_text(render(services))
    return out, (time.perf_counter() - start) * 1000


if __name__ == "__main__":
    out, ms = build()
    print(f"built {out.relative_to(ROOT)} in {ms:.0f} ms")
