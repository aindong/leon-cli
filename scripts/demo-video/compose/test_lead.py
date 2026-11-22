"""lead.py, on a made-up clip: python3 -m unittest discover -s compose (in the venv)."""
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
import lead  # noqa: E402  (scripts/demo-video/lead.py)


class LeadTest(unittest.TestCase):
    def test_the_first_change_sets_the_lead(self):
        """a still screen for 2.5 s, then another: the cue sent ⌘N at 2.0, so the video leads by 0.5"""
        self.lead_for("cmd+n")

    def test_typed_text_sets_it_too(self):
        """the web demo's first keys type a command into a shell"""
        self.lead_for(None)

    def lead_for(self, combo):
        with tempfile.TemporaryDirectory() as d:
            raw, cue = f"{d}/raw.mov", f"{d}/cue.jsonl"
            subprocess.run(["ffmpeg", "-loglevel", "error", "-f", "lavfi", "-i", "color=c=0x201c1a:s=2880x1620:d=2.5:r=30",
                            "-f", "lavfi", "-i", "color=c=0x606060:s=2880x1620:d=1:r=30", "-filter_complex",
                            "[0][1]concat=n=2:v=1", "-c:v", "libx264", "-pix_fmt", "yuv420p", raw], check=True)
            rows = [{"t": 0, "type": "take", "lead": 1.5, "width": 2880, "height": 1620},
                    {"t": 2.0, "type": "key", "combo": combo, "show": True} if combo else
                    {"t": 2.0, "type": "type", "text": "python3 runtests.py"}]
            Path(cue).write_text("".join(json.dumps(r) + "\n" for r in rows))
            out = subprocess.run([sys.executable, lead.__file__, raw, cue], capture_output=True, text=True, check=True)
            first = json.loads(Path(cue).read_text().splitlines()[0])
        self.assertAlmostEqual(float(out.stdout), 0.5, delta=1 / 30 + 1e-6)
        self.assertEqual(first["lead"], float(out.stdout))
        self.assertEqual(first["width"], 2880)  # the rest of the line kept


if __name__ == "__main__":
    unittest.main()
