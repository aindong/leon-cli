"""The web demo's two cuts (compose.py web, web-short): python3 -m unittest discover -s compose."""
import json
import tempfile
import unittest
from pathlib import Path

from compose import END, frame_plan, pieces, srt
from frame import load_cue
from web import VERTICAL, WEB16, WEB4K, WEB_CUT, WEB_FRAMES, WEB_SQUARE, WEB_VO, layout, stacked, vo_place
from frame import Renderer
from PIL import Image

HERE = Path(__file__).resolve().parent
MARKS = ["1 your mac", "2 settings", "3 browser", "4 answer", "5 phone"]


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
        self.assertEqual(ps[-1][:3], ("card", "end", END))
        self.assertEqual(ps[-1][3][0], "Open it anywhere.")  # the end card's headline
        self.assertEqual(pieces("web-4k", None, 60.0, c), ps)  # the 4K cut: the same pieces

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
        self.assertEqual(ps[0][4][0], "Open it anywhere.")  # the hook, on the opening frame
        total = len(frame_plan(ps)) / 60
        self.assertTrue(12 <= total <= 20, total)  # "a ~15 s short cut"

    def test_square_and_vertical_frame_each_piece(self):
        c = load_cue(take())
        short = pieces("web-short", None, 60.0, c)
        for mode, L in (("web-square", WEB_SQUARE), ("web-vertical", VERTICAL)):
            ps = pieces(mode, None, 60.0, c)
            self.assertIs(layout(mode), L)
            self.assertEqual([p[:3] for p in ps[:-1]], [p[:3] for p in short[:-1]])  # the same pieces
            self.assertEqual([p[3] for p in ps[:-1]], WEB_FRAMES[mode])
            for f in WEB_FRAMES[mode]:
                # a camera region (a list) or regions stacked (a tuple), all inside the 4K take
                regions = [f] if isinstance(f, list) else f
                self.assertIsInstance(f, (list, tuple))
                for x, y, w, h in regions:
                    self.assertTrue(0 <= x and x + w <= 3840 and 0 <= y and y + h <= 2160, (x, y, w, h))
                if isinstance(f, tuple) and len(f) > 1:  # stacked edge to edge: they fill the frame
                    self.assertAlmostEqual(sum(L.vw * h / w for _, _, w, h in f), L.vh, delta=4)
        self.assertIs(layout("web-short"), WEB16)
        self.assertIs(layout("web-4k"), WEB4K)
        self.assertEqual((VERTICAL.W, VERTICAL.H, WEB4K.W, WEB4K.H), (1080, 1920, 3840, 2160))
        for L in (WEB16, WEB4K, WEB_SQUARE, VERTICAL):  # the VM's own screen: no frame around it
            self.assertEqual((L.vx, L.vy, L.vw, L.vh), (0, 0, L.W, L.H))

    def test_stacked_is_the_layouts_size_with_both_windows(self):
        src = Image.new("RGB", (3840, 2160), (200, 0, 0))
        src.paste((0, 0, 200), (1920, 0, 3840, 2160))  # the right half, the browser, blue
        R = Renderer(VERTICAL, load_cue(take()))
        img = stacked(R, src, 5.0, 0.5, WEB_FRAMES["web-vertical"][0], (0, 2, "Caption", ""))
        self.assertEqual(img.size, (1080, 1920))
        top = img.getpixel((540, VERTICAL.vy + 400))
        bottom = img.getpixel((540, VERTICAL.vy + VERTICAL.vh - 200))
        self.assertGreater(top[0], top[2])  # the app on top
        self.assertGreater(bottom[2], bottom[0])  # the page under it
        self.assertGreater(img.getpixel((540, 959))[0], 150)  # edge to edge: the app right to the seam
        self.assertGreater(img.getpixel((540, 961))[2], 150)  # and the browser right after it
        # one region narrower than the frame: the screen around it behind, blurred and dark, no
        # brand gradient (whose corners are brown)
        one = stacked(R, src, 5.0, 0.5, ((2200, 1000, 1240, 620),), (0, 2, "Caption", ""))
        top = one.getpixel((540, 100))
        self.assertTrue(top[2] > top[0] and max(top) < 120, top)  # the right half's blue, darkened

    def test_a_take_without_a_mark_of_the_cut_says_which(self):
        c = [r for r in load_cue(take()) if r.get("name") != "3 browser"]
        with self.assertRaises(SystemExit) as e:
            pieces("web-short", None, 60.0, c)
        self.assertIn("3 browser", str(e.exception))

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

    def test_the_voice_over_lands_on_its_beats_and_never_overlaps(self):
        c = load_cue(take())
        lengths = {clip: 1.5 for clip, *_ in WEB_VO["web"] + WEB_VO["web-short"]}
        full = vo_place("web", c, 60.0, lengths)
        marks = {r["name"]: r["vt"] for r in c if r["type"] == "mark"}
        start = pieces("web", None, 60.0, c)[0][1]
        self.assertAlmostEqual(full[0][1], marks["1 your mac"] - start + WEB_VO["web"][0][2])
        for (_, s, e), (_, s2, _) in zip(full, full[1:]):
            self.assertLess(e, s2)
        short = vo_place("web-short", c, 60.0, lengths)
        self.assertEqual([x[0] for x in short], [x[0] for x in WEB_VO["web-short"]])
        self.assertAlmostEqual(short[0][1], 0.15)  # the hook's line on the opening frame
        lengths["web-3.mp3"] = 30.0  # a clip that would talk over the next one stops the render
        with self.assertRaises(SystemExit) as e:
            vo_place("web", c, 60.0, lengths)
        self.assertIn("web-3.mp3", str(e.exception))

    def test_the_web_scenes_mark_every_piece_of_the_short_cut(self):
        scenes = (HERE.parent / "scenes-web.sh").read_text()
        for m, *_ in WEB_CUT:
            self.assertIn(f'mark "{m}"', scenes)


if __name__ == "__main__":
    unittest.main()
