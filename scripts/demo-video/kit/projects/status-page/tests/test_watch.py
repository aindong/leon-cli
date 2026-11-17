import json
import os
import pathlib
import select
import shutil
import subprocess
import sys
import tempfile
import time
import unittest

import build

ROOT = pathlib.Path(__file__).parent.parent


class BigPageTest(unittest.TestCase):
    def test_5000_services_in_under_a_second(self):
        many = [{"name": f"svc-{i}", "status": ["up", "degraded", "down"][i % 3], "latency_ms": i % 500,
                 "uptime": 99.0} for i in range(5000)]
        start = time.perf_counter()
        page = build.render(many)
        self.assertLess(time.perf_counter() - start, 1.0)
        self.assertEqual(page.count('class="service"'), 5000)


class WatchTest(unittest.TestCase):
    """watch.py in a copy of the project: edit services.json, see the page rebuilt."""

    def line(self, proc, seconds=10):
        ready, _, _ = select.select([proc.stdout], [], [], seconds)
        self.assertTrue(ready, f"watch.py said nothing in {seconds} s")
        return proc.stdout.readline()

    def test_an_edit_rebuilds_the_page(self):
        with tempfile.TemporaryDirectory() as d:
            for f in ["build.py", "watch.py", "services.json"]:
                shutil.copy(ROOT / f, d)
            proc = subprocess.Popen([sys.executable, "-u", "watch.py", "0"], cwd=d, stdout=subprocess.PIPE, text=True)
            try:
                self.assertIn("built index.html", self.line(proc))
                services = json.loads((pathlib.Path(d) / "services.json").read_text())
                services[0]["status"] = "down"
                time.sleep(0.05)  # a new mtime, even on a coarse clock
                tmp = pathlib.Path(d) / "services.json.tmp"  # replaced whole: the watcher never reads half a file
                tmp.write_text(json.dumps(services))
                os.replace(tmp, pathlib.Path(d) / "services.json")
                self.assertIn("rebuilt", self.line(proc))
                page = (pathlib.Path(d) / "site" / "index.html").read_text()
                self.assertIn(f'<span>{services[0]["name"]}</span><span class="down">', page)
            finally:
                proc.terminate()
                proc.wait(5)
                proc.stdout.close()
