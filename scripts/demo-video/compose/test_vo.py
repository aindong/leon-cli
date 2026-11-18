"""The voice-over's placement and mix: python3 -m unittest discover -s compose (in the venv)."""
import subprocess
import tempfile
import unittest
from pathlib import Path

from compose import VO_LEAD, VO_SCENES, audio_args, pieces, place, seconds, vo_clips, vo_windows, voice_track
from test_coldopen import cue_file
from frame import load_cue

CUT = (105.1, 107.7)
MARKS = [(4.0, "2 build-up"), (17.5, "3 agent runs its tests"), (26.0, "4 activity"), (37.0, "5 speed"),
         (48.7, "5 overview"), (52.5, "6 switch"), (60.5, "7 settings"), (69.5, "8 leon do"), (77.0, "9 tailscale"),
         (91.0, "10 attach"), (96.6, "10 banner"), (101.5, "11 quit"), (112.0, "12 overview")]
CUE = load_cue(cue_file([{"t": t, "type": "mark", "name": n} for t, n in MARKS]))


class WindowsTest(unittest.TestCase):
    def test_main_has_a_window_a_scene_in_order_to_the_end(self):
        ws = vo_windows("main", CUT, 117.0, CUE)
        self.assertEqual(len(ws), len(VO_SCENES))
        self.assertEqual(ws[0][0], 0.0)
        self.assertTrue(all(a < b for a, b in ws))
        self.assertTrue(all(ws[i][1] == ws[i + 1][0] for i in range(len(ws) - 1)))
        ps = pieces("main", CUT, 117.0, CUE)
        self.assertAlmostEqual(ws[1][0], ps[0][2] - ps[0][1])  # the empty state's line starts at the cold open's cut
        self.assertAlmostEqual(ws[-1][1] - ws[-1][0], ps[-1][2])  # the last line is the end card's

    def test_square_ends_before_the_end_card(self):
        (a, b), = vo_windows("square", None, 117.0, CUE)
        self.assertEqual(b, 30.0 - pieces("square", None, 117.0, CUE)[-1][2])


class PlaceTest(unittest.TestCase):
    def test_no_clip_overlaps_the_next(self):
        ws = vo_windows("main", CUT, 117.0, CUE)
        full = {i: b - a for i, (a, b) in enumerate(ws)}  # each clip as long as its scene: the most that fits
        full[len(ws) - 1] -= VO_LEAD  # the last ends with the video
        placed = place(ws, full)
        self.assertEqual([p[0] for p in placed], list(range(len(ws))))
        for (_, s, e), (_, s2, _) in zip(placed, placed[1:]):
            self.assertLessEqual(e, s2 + 1e-9)
        self.assertLessEqual(placed[-1][2], ws[-1][1] + 1e-9)  # the last ends with the video
        self.assertAlmostEqual(placed[3][1], ws[3][0] + VO_LEAD)

    def test_a_clip_too_long_for_its_scene_stops_the_render(self):
        ws = vo_windows("main", CUT, 117.0, CUE)
        with self.assertRaises(SystemExit):
            place(ws, {3: ws[3][1] - ws[3][0] + 0.01})

    def test_the_error_names_the_clip_that_runs_long(self):
        ws = vo_windows("square", None, 117.0, CUE)
        with self.assertRaises(SystemExit) as e:
            place(ws, {0: 30.0}, {0: "square.mp3"})
        self.assertIn("square.mp3", str(e.exception))

    def test_a_take_without_a_scenes_mark_is_named_not_a_traceback(self):
        partial = load_cue(cue_file([{"t": t, "type": "mark", "name": n} for t, n in MARKS[:5]]))
        with self.assertRaises(SystemExit) as e:
            vo_windows("main", CUT, 117.0, partial)
        self.assertIn("6 switch", str(e.exception))

    def test_square_clip_must_end_before_the_end_card(self):
        ws = vo_windows("square", None, 117.0, CUE)
        room = ws[0][1] - VO_LEAD
        self.assertLessEqual(place(ws, {0: room})[0][2], ws[0][1])
        with self.assertRaises(SystemExit):
            place(ws, {0: room + 0.01})


class SilentTest(unittest.TestCase):
    def test_no_vo_folder_is_todays_silent_track(self):
        self.assertEqual(vo_clips("main", None), {})
        with tempfile.TemporaryDirectory() as d:
            self.assertEqual(vo_clips("square", d), {})  # a folder without its clips: silent too
        self.assertEqual(audio_args(None), (["-f", "lavfi", "-i", "anullsrc=r=48000:cl=stereo"], []))


class MixTest(unittest.TestCase):
    def test_the_clips_land_at_their_places_at_the_target_loudness(self):
        with tempfile.TemporaryDirectory() as d:
            clips = {}
            for i in (0, 2):
                clips[i] = Path(d) / f"main-{i:02}.mp3"
                subprocess.run(["ffmpeg", "-loglevel", "error", "-f", "lavfi", "-i", "sine=f=220:d=4",
                                "-af", "volume=-20dB", str(clips[i])], check=True)
            self.assertEqual(sorted(vo_clips("main", d)), [0, 2])
            wav = Path(d) / "voice.wav"
            norm = voice_track([(0, 0.3, 4.3), (2, 6.0, 10.0)], clips, 12.0, wav)
            self.assertAlmostEqual(seconds(wav), 12.0, delta=0.05)
            out = Path(d) / "out.m4a"
            subprocess.run(["ffmpeg", "-loglevel", "error", "-i", str(wav), "-af", norm, "-c:a", "aac", str(out)],
                           check=True)
            err = subprocess.run(["ffmpeg", "-hide_banner", "-i", str(out), "-af", "ebur128=peak=true", "-f", "null",
                                  "-"], capture_output=True, text=True).stderr
            summary = err[err.rindex("Summary:"):]
            loud = float(summary.split("I:")[1].split()[0])
            peak = float(summary.split("Peak:")[1].split()[0])
            self.assertAlmostEqual(loud, -16.0, delta=1.0)
            self.assertLess(peak, -1.0)
            # silent between the clips: 4.5 to 5.8 s holds nothing
            gap = subprocess.run(["ffmpeg", "-hide_banner", "-ss", "4.5", "-t", "1.3", "-i", str(wav), "-af",
                                  "volumedetect", "-f", "null", "-"], capture_output=True, text=True).stderr
            self.assertIn("max_volume: -91", gap)


if __name__ == "__main__":
    unittest.main()
