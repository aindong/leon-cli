import json
import threading
import unittest
import urllib.error
import urllib.request

from shelf.app import serve
from shelf.store import Store


class ApiCase(unittest.TestCase):
    """The API over real HTTP, on a port the OS picks."""

    @classmethod
    def setUpClass(cls):
        cls.store = Store()
        for i in range(30):
            cls.store.add(f"Book {i}", "Author", 1990 + i, "software" if i % 3 else "fiction")
        cls.httpd = serve(0, cls.store, log=lambda line: None)
        cls.base = f"http://127.0.0.1:{cls.httpd.server_address[1]}"
        threading.Thread(target=cls.httpd.serve_forever, daemon=True).start()

    @classmethod
    def tearDownClass(cls):
        cls.httpd.shutdown()

    def call(self, path, method="GET", body=None):
        data = json.dumps(body).encode() if body is not None else None
        req = urllib.request.Request(self.base + path, data=data, method=method)
        try:
            with urllib.request.urlopen(req, timeout=5) as r:
                raw = r.read()
                return r.status, json.loads(raw) if raw else {}
        except urllib.error.HTTPError as e:
            raw = e.read()
            return e.code, json.loads(raw) if raw else {}


class HttpTest(ApiCase):
    def test_health(self):
        code, body = self.call("/health")
        self.assertEqual(code, 200)
        self.assertTrue(body["ok"])

    def test_get_one_book(self):
        code, body = self.call("/books/3")
        self.assertEqual((code, body["title"]), (200, "Book 2"))

    def test_missing_book_is_404(self):
        self.assertEqual(self.call("/books/999")[0], 404)

    def test_filter_by_shelf(self):
        _, body = self.call("/books?shelf=fiction")
        self.assertTrue(all(b["shelf"] == "fiction" for b in body["books"]))

    def test_post_creates_a_book(self):
        code, body = self.call("/books", "POST", {"title": "New", "author": "Writer"})
        self.assertEqual(code, 201)
        self.assertEqual(self.call(f"/books/{body['id']}")[1]["title"], "New")

    def test_post_of_a_json_list_is_400(self):
        self.assertEqual(self.call("/books", "POST", [])[0], 400)

    def test_post_without_a_title_is_400(self):
        self.assertEqual(self.call("/books", "POST", {"author": "Writer"})[0], 400)

    def test_delete(self):
        _, body = self.call("/books", "POST", {"title": "Temp", "author": "Writer"})
        self.assertEqual(self.call(f"/books/{body['id']}", "DELETE")[0], 204)
        self.assertEqual(self.call(f"/books/{body['id']}")[0], 404)

    def test_unknown_route_is_404(self):
        self.assertEqual(self.call("/shelves")[0], 404)
