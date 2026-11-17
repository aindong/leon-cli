"""The compositor's arithmetic: python3 -m unittest discover -s compose (in the venv)."""
import json
import math
import tempfile
import unittest

from compose import SQUARE_CUT, frame_plan, pieces, soft, srt
from draw import combo_caps, ease
from frame import BASE, MAIN, Renderer, fit, lerp_cam, load_cue


def cue(*zooms):
    rows = [{"t": 0, "type": "take", "lead": 1.5}]
    rows += [{"t": a, "type": "zoom", "region": r, "until": b} for a, r, b in zooms]
    with tempfile.NamedTemporaryFile("w", suffix=".jsonl") as f:
        f.write("\n".join(json.dumps(r) for r in rows))
        f.flush()
        return load_cue(f.name)


def close(a, b):
    return all(abs(x - y) < 1e-6 for x, y in zip(a, b))


class EaseTest(unittest.TestCase):
    def test_ends_and_middle(self):
        self.assertEqual((ease(0), ease(1), ease(-1), ease(2)), (0, 1, 0, 1))
        self.assertAlmostEqual(ease(0.5), 0.5)


class FitTest(unittest.TestCase):
    def test_stays_inside_the_frame_and_under_the_cap(self):
        aspect = MAIN.vw / MAIN.vh
        for region in ([0, 0, 10, 10], [2800, 1500, 40, 40], [0, 9, 2848, 1602], [1470, 960, 1378, 640]):
            cx, cy, w = fit(region, aspect, max_zoom=BASE[2] / MAIN.vw)
            h = w / aspect
            self.assertGreaterEqual(cx - w / 2, BASE[0] - 1e-6)
            self.assertLessEqual(cx + w / 2, BASE[0] + BASE[2] + 1e-6)
            self.assertGreaterEqual(cy - h / 2, BASE[1] - 1e-6)
            self.assertLessEqual(cy + h / 2, BASE[1] + BASE[3] + 1e-6)
            self.assertGreaterEqual(w, MAIN.vw - 1e-6)  # never past one raw pixel per output pixel

    def test_lerp_ends(self):
        a, b = (100, 100, 2000), (500, 400, 1000)
        self.assertTrue(close(lerp_cam(a, b, 0), a))
        self.assertAlmostEqual(lerp_cam(a, b, 1)[2], 1000)


class CameraTest(unittest.TestCase):
    def path(self, R, t0, t1):
        return [R.camera(t0 + i / 60) for i in range(int((t1 - t0) * 60))]

    def assert_smooth(self, cams):
        for a, b in zip(cams, cams[1:]):
            self.assertLess(abs(b[2] - a[2]), 60, "a zoom step jumped")
            self.assertLess(math.hypot(b[0] - a[0], b[1] - a[1]), 60, "a pan step jumped")

    def test_home_before_and_after_a_beat(self):
        R = Renderer(MAIN, cue((2.0, [0, 9, 1600, 900], 4.0)))
        self.assertTrue(close(R.camera(0.5), R.home))
        self.assertTrue(close(R.camera(10.0), R.home))
        self.assert_smooth(self.path(R, 3.0, 7.0))

    def test_close_beats_chain_without_going_home(self):
        R = Renderer(MAIN, cue((2.0, [0, 9, 1768, 990], 4.0), (4.3, [0, 9, 1714, 720], 6.0)))
        self.assertEqual(len(R.moves), 3)  # in, across, home: no home between the two
        self.assert_smooth(self.path(R, 3.0, 9.0))


class TimelineTest(unittest.TestCase):
    def test_square_is_thirty_seconds(self):
        self.assertEqual(len(frame_plan(pieces("square", None, 106.0))), 30 * 60)

    def test_square_pieces_run_forward_and_leave_the_end_card_five_seconds(self):
        for (s0, e0, *_), (s1, *_) in zip(SQUARE_CUT, SQUARE_CUT[1:]):
            self.assertLess(s0, e0)
            self.assertLessEqual(e0, s1)
        self.assertGreaterEqual(pieces("square", None, 106.0)[-1][2], 5.0)

    def test_main_cut_removes_the_gap(self):
        whole = len(frame_plan(pieces("main", None, 106.0)))
        cut = len(frame_plan(pieces("main", (93.0, 96.0), 106.0)))
        self.assertEqual(whole - cut, 3 * 60)

    def test_only_the_quit_join_is_hard(self):
        self.assertEqual([soft("main", i) for i in range(4)], [False, True, False, True])
        self.assertTrue(all(soft("square", i) for i in range(1, len(SQUARE_CUT) + 1)))


class SrtTest(unittest.TestCase):
    def test_a_caption_running_into_the_cut_is_clipped_not_dropped(self):
        rows = [{"t": 0, "type": "take", "lead": 1.5},
                {"t": 91, "type": "caption", "text": "Quit the app", "until": 93},
                {"t": 95, "type": "caption", "text": "Still running", "until": 98}]
        with tempfile.TemporaryDirectory() as d:
            cue_path, out = f"{d}/c.jsonl", f"{d}/o.srt"
            with open(cue_path, "w") as f:
                f.write("\n".join(json.dumps(r) for r in rows))
            srt("main", cue_path, (94.0, 96.85), 106.0, out)
            text = open(out).read()
        self.assertIn("Quit the app", text)  # 92.5-94.5 in video time: kept up to the cut at 94
        self.assertIn("00:01:35,000 --> 00:01:36,500", text)  # title 2.5 + 92.5, to 2.5 + 94
        self.assertIn("00:01:36,500 --> 00:01:39,150", text)  # 96.85-99.5, shifted back by the 2.85 s cut


class KeysTest(unittest.TestCase):
    def test_macos_modifier_order(self):
        self.assertEqual(combo_caps("cmd+shift+\\"), ["shift", "cmd", "\\"])
        self.assertEqual(combo_caps("cmd+alt+left"), ["alt", "cmd", "left"])


if __name__ == "__main__":
    unittest.main()
