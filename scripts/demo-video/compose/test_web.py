"""The web demo's two cuts (compose.py web, web-short): python3 -m unittest discover -s compose."""
import json
import tempfile
import unittest
from pathlib import Path

from compose import END, frame_plan, pieces, srt
from frame import load_cue
from web import VERTICAL, WEB_CUT, WEB_FRAMES, layout, stacked
from frame import SQUARE, MAIN, Renderer
from PIL import Image

HERE = Path(__file__).resolve().parent
MARKS = ["1 your mac", "2 settings", "3 laptop", "4 typing", "5 blocked", "6 phone"]


def take(lead=1.2, captions=()):
    """a cue like a web take's: its marks 8 s apart, from 0.5 s, and the captions given"""
    rows = [{"t": 0, "type": "take", "lead": lead}]
    rows += [{"t": 0.5 + 8 * i, "type": "mark", "name": m} for i, m in enumerate(MARKS)]
    rows += [{"t": a, "type": "caption", "text": c, "until": b, "small": ""} for a, b, c in captions]
    f = tempfile.NamedTemporaryFile("w", suffix=".jsonl", delete=False)
    f.write("\n".join(json.dumps(r) for r in rows))
    f.close()
    return f.name


class WebTest(unittest.TestCase):
    def test_the_whole_take_from_its_first_mark_then_the_end_card(self):
        c = load_cue(take())
        ps = pieces("web", None, 60.0, c)
        self.assertEqual(ps[0][0], "take")
        self.assertAlmostEqual(ps[0][1], 0.5 + 1.2 - 0.3)  # just before the first mark, in video time
        self.assertEqual(ps[0][2], 60.0)
        self.assertEqual(ps[-1], ("card", "end", END))

    def test_the_short_cut_is_its_pieces_by_mark_with_their_own_captions(self):
        c = load_cue(take())
        ps = pieces("web-short", None, 60.0, c)
        marks = {r["name"]: r["vt"] for r in c if r["type"] == "mark"}
        for p, (m, off, dur, cap, small) in zip(ps, WEB_CUT):
            self.assertEqual(p[0], "take")
            self.assertAlmostEqual(p[1], marks[m] + off)
            self.assertAlmostEqual(p[2] - p[1], dur)
            self.assertIs(p[3], WEB_FRAMES["web-short"][WEB_CUT.index((m, off, dur, cap, small))])
            self.assertEqual(p[4], (cap, small))
        self.assertEqual(ps[-1][:2], ("card", "end"))
        total = len(frame_plan(ps)) / 60
        self.assertTrue(12 <= total <= 20, total)  # "a ~15 s short cut"

    def test_square_and_vertical_frame_each_piece(self):
        c = load_cue(take())
        short = pieces("web-short", None, 60.0, c)
        for mode, L in (("web-square", SQUARE), ("web-vertical", VERTICAL)):
            ps = pieces(mode, None, 60.0, c)
            self.assertIs(layout(mode), L)
            self.assertEqual([p[:3] for p in ps[:-1]], [p[:3] for p in short[:-1]])  # the same pieces
            self.assertEqual([p[3] for p in ps[:-1]], WEB_FRAMES[mode])
            for regions in WEB_FRAMES[mode]:
                self.assertIsInstance(regions, tuple)  # compose.py tells them from a camera region so
                for x, y, w, h in regions:
                    self.assertTrue(0 <= x and x + w <= 2880 and 0 <= y and y + h <= 1620, (x, y, w, h))
        self.assertIs(layout("web-short"), MAIN)
        self.assertEqual((VERTICAL.W, VERTICAL.H), (1080, 1920))

    def test_stacked_is_the_layouts_size_with_both_windows(self):
        src = Image.new("RGB", (2880, 1620), (200, 0, 0))
        src.paste((0, 0, 200), (1440, 0, 2880, 1620))  # the right half, the page, blue
        R = Renderer(VERTICAL, load_cue(take()))
        img = stacked(R, src, 5.0, 0.5, WEB_FRAMES["web-vertical"][0], (0, 2, "Caption", ""))
        self.assertEqual(img.size, (1080, 1920))
        top = img.getpixel((540, VERTICAL.vy + 400))
        bottom = img.getpixel((540, VERTICAL.vy + VERTICAL.vh - 200))
        self.assertGreater(top[0], top[2])  # the app on top
        self.assertGreater(bottom[2], bottom[0])  # the page under it
        one = stacked(R, src, 5.0, 0.5, ((2000, 76, 800, 1512),), (0, 2, "Caption", ""))
        self.assertEqual(one.getpixel((540, 960))[2], 200)  # one tall region, scaled to fit, centred
        self.assertNotEqual(one.getpixel((40, 960))[2], 200)  # narrower than the frame: background beside it

    def test_a_take_without_a_mark_of_the_cut_says_which(self):
        c = [r for r in load_cue(take()) if r.get("name") != "4 typing"]
        with self.assertRaises(SystemExit) as e:
            pieces("web-short", None, 60.0, c)
        self.assertIn("4 typing", str(e.exception))

    def test_srt_lines_are_on_the_outputs_clock(self):
        path = take(captions=[(1.0, 6.0, "Your Mac: an agent at work.")])
        with tempfile.TemporaryDirectory() as d:
            srt("web", path, None, 60.0, f"{d}/web.srt")
            text = Path(f"{d}/web.srt").read_text()
            srt("web-short", path, None, 60.0, f"{d}/short.srt")
            short = Path(f"{d}/short.srt").read_text()
        # the caption at script 1.0 is video 2.2, and the output starts at video 1.4
        self.assertIn("00:00:00,800 --> 00:00:05,800\nYour Mac: an agent at work.", text)
        self.assertIn(WEB_CUT[0][3], short)
        self.assertTrue(short.startswith("1\n00:00:00,000 --> "))

    def test_the_web_scenes_mark_every_piece_of_the_short_cut(self):
        scenes = (HERE.parent / "scenes-web.sh").read_text()
        for m, *_ in WEB_CUT:
            self.assertIn(f'mark "{m}"', scenes)


if __name__ == "__main__":
    unittest.main()
