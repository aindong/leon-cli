"""watch.py [PORT]: serve site/ and rebuild it whenever a source file changes."""
import functools
import http.server
import pathlib
import sys
import threading
import time

import build

ROOT = pathlib.Path(__file__).parent
SOURCES = [ROOT / "services.json", ROOT / "build.py"]


class Quiet(http.server.SimpleHTTPRequestHandler):
    def log_message(self, fmt, *args):
        print(time.strftime("%H:%M:%S"), self.command, self.path, args[1], flush=True)


def main():
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 5173
    handler = functools.partial(Quiet, directory=str(ROOT / "site"))
    httpd = http.server.ThreadingHTTPServer(("127.0.0.1", port), handler)
    threading.Thread(target=httpd.serve_forever, daemon=True).start()
    seen = {p: p.stat().st_mtime for p in SOURCES}  # before the first line: an edit after it is seen
    out, ms = build.build()
    print(f"status-page  http://127.0.0.1:{port}  (built {out.name} in {ms:.0f} ms)", flush=True)
    while True:
        time.sleep(0.3)
        for p in SOURCES:
            try:
                mtime = p.stat().st_mtime
            except FileNotFoundError:  # mid-save by an editor that replaces the file: next poll
                continue
            if mtime != seen[p]:
                seen[p] = mtime
                try:
                    import importlib
                    importlib.reload(build)
                    out, ms = build.build()
                    print(time.strftime("%H:%M:%S"), f"{p.name} changed: rebuilt in {ms:.0f} ms", flush=True)
                except Exception as e:
                    print(time.strftime("%H:%M:%S"), f"build failed: {e}", flush=True)


if __name__ == "__main__":
    main()
