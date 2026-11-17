import json
import pathlib
import unittest

import build

SERVICES = json.loads((pathlib.Path(build.__file__).parent / "services.json").read_text())


class RenderTest(unittest.TestCase):
    def setUp(self):
        self.page = build.render(SERVICES)

    def test_every_service_has_a_row(self):
        self.assertEqual(self.page.count('class="service"'), len(SERVICES))

    def test_headline_names_the_worst_status(self):
        self.assertIn('<h1 class="down">Partial outage</h1>', self.page)

    def test_all_up(self):
        up = [dict(s, status="up") for s in SERVICES]
        self.assertIn("All systems normal", build.render(up))

    def test_names_are_escaped(self):
        page = build.render([{"name": "<script>", "status": "up", "latency_ms": 1, "uptime": 100}])
        self.assertIn("&lt;script&gt;", page)
        self.assertNotIn("<script>", page)

    def test_missing_latency_shows_a_dash(self):
        self.assertIn("<span>—</span>", self.page)

    def test_build_writes_the_page(self):
        out, ms = build.build()
        self.assertTrue(out.read_text().startswith("<!doctype html>"))
