import pathlib
import subprocess
import sys
import tempfile
import unittest

import genlog

ROOT = pathlib.Path(__file__).parent.parent


class CliTest(unittest.TestCase):
    """logtally.py as a command, on a generated log file."""

    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory()
        cls.log = pathlib.Path(cls.tmp.name) / "access.log"
        cls.log.write_text("".join(genlog.lines(300_000, seed=3)))

    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup()

    def run_cli(self, *args):
        return subprocess.run([sys.executable, "logtally.py", *args], cwd=ROOT, capture_output=True, text=True, check=True).stdout

    def test_summary_of_300000_lines(self):
        self.assertIn("requests  300,000", self.run_cli(str(self.log)))

    def test_top_three_paths(self):
        out = self.run_cli(str(self.log), "--top", "3")
        self.assertEqual(sum("█" in line for line in out.splitlines()), 3)

    def test_reads_stdin(self):
        out = subprocess.run([sys.executable, "logtally.py", "-"], cwd=ROOT, input="GET / 200 1ms\n",
                             capture_output=True, text=True, check=True).stdout
        self.assertIn("requests  1", out)
