"""shelf-api: a JSON API for a reading list."""
import json
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlparse

from .store import Store


def make_handler(store, log=print):
    class Handler(BaseHTTPRequestHandler):
        def send_json(self, code, body):
            data = json.dumps(body).encode()
            self.send_response(code)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)
            self.code = code

        def route(self, method):
            start = time.perf_counter()
            url = urlparse(self.path)
            parts = [p for p in url.path.split("/") if p]
            try:
                self.dispatch(method, parts, parse_qs(url.query))
            except ValueError as e:
                self.send_json(400, {"error": str(e)})
            ms = (time.perf_counter() - start) * 1000
            log(f"{method:<6} {url.path:<14} {getattr(self, 'code', 500)} {ms:.0f}ms")

        def dispatch(self, method, parts, query):
            if parts == ["health"] and method == "GET":
                return self.send_json(200, {"ok": True, "books": store.count()})
            if parts == ["books"] and method == "GET":
                shelf = query.get("shelf", [None])[0]
                limit = int(query.get("limit", ["20"])[0])
                offset = int(query.get("offset", ["0"])[0])
                if not 1 <= limit <= 100 or offset < 0:
                    raise ValueError("limit is 1-100, offset 0 or more")
                books = store.all(shelf)
                page = books[offset:offset + limit]
                more = offset + limit < len(books)
                return self.send_json(200, {"books": page, "total": len(books),
                                            "next": offset + limit if more else None})
            if parts == ["books"] and method == "POST":
                n = int(self.headers.get("Content-Length", 0))
                body = json.loads(self.rfile.read(n) or b"{}")
                book = store.add(body.get("title"), body.get("author"), body.get("year"),
                                 body.get("shelf", "unsorted"))
                return self.send_json(201, book)
            if len(parts) == 2 and parts[0] == "books" and parts[1].isdigit():
                book_id = int(parts[1])
                if method == "GET":
                    book = store.get(book_id)
                    return self.send_json(200, book) if book else self.send_json(404, {"error": "not found"})
                if method == "DELETE":
                    return self.send_json(204 if store.remove(book_id) else 404, {})
            self.send_json(404, {"error": "no such route"})

        def do_GET(self):
            self.route("GET")

        def do_POST(self):
            self.route("POST")

        def do_DELETE(self):
            self.route("DELETE")

        def log_message(self, *args):
            pass

    return Handler


class Server(ThreadingHTTPServer):
    request_queue_size = 128  # the default 5 resets connections when clients arrive together


def serve(port=8080, store=None, log=print):
    store = store or Store()
    return Server(("127.0.0.1", port), make_handler(store, log))
