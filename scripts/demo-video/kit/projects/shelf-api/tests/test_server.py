import json
import pathlib
import socket
import subprocess
import sys
import unittest
import urllib.request
from concurrent.futures import ThreadPoolExecutor

ROOT = pathlib.Path(__file__).parent.parent


def free_port():
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


class ServerProcessTest(unittest.TestCase):
    """serve.py as it runs in production: its own process, a real port."""

    def setUp(self):
        self.port = free_port()
        self.proc = subprocess.Popen([sys.executable, "-u", "serve.py", str(self.port)], cwd=ROOT,
                                     stdout=subprocess.PIPE, text=True)
        self.assertIn("listening", self.proc.stdout.readline())

    def tearDown(self):
        self.proc.terminate()
        self.proc.wait(5)
        self.proc.stdout.close()

    def get(self, path):
        with urllib.request.urlopen(f"http://127.0.0.1:{self.port}{path}", timeout=5) as r:
            return json.loads(r.read())

    def test_starts_with_the_seed_books(self):
        self.assertEqual(self.get("/health")["books"], 5)

    def test_serves_500_requests_from_16_clients(self):
        with ThreadPoolExecutor(16) as pool:
            results = list(pool.map(lambda i: self.get(f"/books/{i % 5 + 1}"), range(500)))
        self.assertEqual(len({r["id"] for r in results}), 5)

    def test_stops_cleanly(self):
        self.proc.terminate()
        self.assertIsNotNone(self.proc.wait(5))
