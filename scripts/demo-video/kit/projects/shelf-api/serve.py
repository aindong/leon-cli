"""Run shelf-api on a port with a few books, logging each request."""
import sys
import time

from shelf.app import serve
from shelf.store import Store

SEED = [
    ("The Left Hand of Darkness", "Ursula K. Le Guin", 1969, "fiction"),
    ("A Pattern Language", "Christopher Alexander", 1977, "design"),
    ("The Mythical Man-Month", "Frederick P. Brooks Jr.", 1975, "software"),
    ("Thinking in Systems", "Donella H. Meadows", 2008, "systems"),
    ("Structure and Interpretation of Computer Programs", "Abelson and Sussman", 1985, "software"),
]

if __name__ == "__main__":
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8080
    store = Store()
    for book in SEED:
        store.add(*book)
    stamp = lambda line: print(time.strftime("%H:%M:%S"), line, flush=True)
    httpd = serve(port, store, stamp)
    print(f"shelf-api listening on http://127.0.0.1:{port}  ({store.count()} books)", flush=True)
    httpd.serve_forever()
