"""The main cut's cold open: python3 -m unittest discover -s compose (in the venv)."""
import json
import os
import tempfile
import unittest

from compose import COLD, TITLE, frame_plan, pieces, soft, srt
from frame import FPS, load_cue


def cue_file(rows):
    f = tempfile.NamedTemporaryFile("w", suffix=".jsonl", delete=False)
    f.write("\n".join(json.dumps(r) for r in [{"t": 0, "type": "take", "lead": 1.5}] + rows))
    f.close()
    unittest.addModuleCleanup(os.unlink, f.name)
    return f.name


OVERVIEW = {"t": 48.5, "type": "mark", "name": "5 overview"}


class ColdOpenTest(unittest.TestCase):
    def test_opens_on_the_overview_then_cuts_to_the_empty_state(self):
        ps = pieces("main", (103.5, 105.6), 117.0, load_cue(cue_file([OVERVIEW])))
        cold, story = ps[0], ps[1]
        self.assertEqual(cold[0], "take")
        self.assertAlmostEqual(cold[1], 48.5 + 1.5 + 0.4)  # just after the overview opens, video time
        self.assertAlmostEqual(cold[2] - cold[1], COLD)
        self.assertEqual(story[1], 0.0)  # then the story from its first frame: the empty state
        self.assertNotIn(("card", "title", TITLE), ps)  # the empty state is the title
        self.assertFalse(soft("main", 1, ps))  # a cut, not a dissolve
        self.assertFalse(soft("main", 2, ps))  # the quit stays a cut
        self.assertTrue(soft("main", 3, ps))  # the end card still dissolves in

    def test_ends_before_the_overview_closes(self):
        """a slow take opens the overview late: the cold open stops at its esc, not after"""
        esc = {"t": 51.8, "type": "key", "combo": "esc", "show": True}
        cold = pieces("main", None, 117.0, load_cue(cue_file([OVERVIEW, esc])))[0]
        self.assertAlmostEqual(cold[2], 51.8 + 1.5 - 0.05)

    def test_an_overview_too_short_to_open_on_keeps_the_title_card(self):
        """its esc within a second of the mark: no cold open, never one past the esc"""
        rows = [{"t": 51.7, "type": "mark", "name": "5 overview"},
                {"t": 51.7, "type": "key", "combo": "cmd+shift+\\", "show": True},
                {"t": 52.0, "type": "key", "combo": "esc", "show": True},
                {"t": 52.5, "type": "key", "combo": "cmd+k", "show": True}]
        self.assertEqual(pieces("main", None, 117.0, load_cue(cue_file(rows)))[0], ("card", "title", TITLE))

    def test_without_the_mark_the_title_card_stays(self):
        ps = pieces("main", None, 106.0, load_cue(cue_file([])))
        self.assertEqual(ps[0], ("card", "title", TITLE))
        self.assertTrue(soft("main", 1, ps))

    def test_captions_shift_by_the_cold_open(self):
        path = cue_file([OVERVIEW, {"t": 4.0, "type": "caption", "text": "Start", "until": 6.0}])
        with tempfile.TemporaryDirectory() as d:
            srt("main", path, None, 117.0, f"{d}/o.srt")
            text = open(f"{d}/o.srt").read()
        self.assertIn("00:00:08,500 --> 00:00:10,500", text)  # 3 s cold open + 5.5 s video time
        self.assertEqual(text.count("Start"), 1)

    def test_frame_count(self):
        ps = pieces("main", None, 117.0, load_cue(cue_file([OVERVIEW])))
        self.assertEqual(len(frame_plan(ps)), int(round((COLD + 117.0 + 6.0) * FPS)))


if __name__ == "__main__":
    unittest.main()
