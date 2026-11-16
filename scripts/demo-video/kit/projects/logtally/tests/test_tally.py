import io
import unittest
from contextlib import redirect_stdout

import genlog
import logtally

LOG = """GET /books 200 12.0ms
GET /books?shelf=a 200 8.0ms
POST /login 401 30.0ms
GET /search 500 240.0ms
not a log line
GET /health 200 1.0ms
"""


class ParseTest(unittest.TestCase):
    def test_a_line(self):
        self.assertEqual(logtally.parse("GET /books 200 12.5ms"), ("GET", "/books", 200, 12.5))

    def test_the_query_is_dropped(self):
        self.assertEqual(logtally.parse("GET /books?x=1 200 1ms")[1], "/books")

    def test_a_leading_time_is_skipped(self):
        self.assertEqual(logtally.parse("20:18:01 GET /books 200 3ms")[1:3], ("/books", 200))

    def test_a_bad_number_is_none(self):
        self.assertIsNone(logtally.parse("GET /books 200 badms"))

    def test_junk_is_none(self):
        self.assertIsNone(logtally.parse("hello world"))
        self.assertIsNone(logtally.parse("GET /x abc 1ms"))


class TallyTest(unittest.TestCase):
    def setUp(self):
        self.t = logtally.tally(LOG.splitlines())

    def test_counts_requests_and_skips_junk(self):
        self.assertEqual((self.t["requests"], self.t["skipped"]), (5, 1))

    def test_only_5xx_are_errors(self):
        self.assertAlmostEqual(self.t["error_rate"], 0.2)

    def test_status_classes(self):
        self.assertEqual(self.t["statuses"], {2: 3, 4: 1, 5: 1})

    def test_percentiles(self):
        self.assertEqual(self.t["p50"], 12.0)
        self.assertGreater(self.t["p99"], self.t["p95"])

    def test_top_paths(self):
        self.assertEqual(self.t["top"][0], ("/books", 2))

    def test_empty_log(self):
        t = logtally.tally([])
        self.assertEqual((t["requests"], t["error_rate"], t["p50"]), (0, 0.0, 0.0))


class BigLogTest(unittest.TestCase):
    """200,000 generated lines: the shape the real logs have."""

    def test_two_hundred_thousand_lines(self):
        t = logtally.tally(genlog.lines(200_000))
        self.assertEqual(t["requests"], 200_000)
        self.assertLess(t["error_rate"], 0.01)
        self.assertEqual(t["top"][0][0], "/books")
        self.assertGreater(t["p99"], t["p50"] * 3)


    def test_a_million_lines(self):
        t = logtally.tally(genlog.lines(1_000_000, seed=11))
        self.assertEqual(t["requests"], 1_000_000)
        self.assertEqual(t["statuses"][2] + t["statuses"][4] + t["statuses"][5], 1_000_000)


class RenderTest(unittest.TestCase):
    def test_plain_output(self):
        out = logtally.render(logtally.tally(LOG.splitlines()), color=False)
        self.assertIn("requests  5", out)
        self.assertIn("errors  20.00%", out)
        self.assertNotIn("\033[", out)

    def test_cli_reads_a_file(self):
        buf = io.StringIO()
        with redirect_stdout(buf):
            logtally.main(["/dev/null"])
        self.assertIn("requests  0", buf.getvalue())
