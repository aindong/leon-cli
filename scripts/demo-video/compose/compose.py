"""compose.py: a raw take + its cue file -> the finished video.

  compose.py main   RAW CUE OUT.mp4 [--cut=A:B] [--frames=F0:F1] [--workers=N] [--blur] [--vo=DIR]
  compose.py square RAW CUE OUT.mp4 [--frames=F0:F1] [--workers=N] [--vo=DIR]
  compose.py stills MODE RAW CUE OUTDIR [--cut=A:B] T...

main: 16:9 1920x1080 at 60 fps: a cold open (COLD seconds of the 50-tab overview, cut straight to the
empty state; a title card instead when the cue has no "5 overview" mark), the take (with the stretch A..B of video time cut
out), an end card; eased zooms onto the cue's zoom beats, a drawn pointer gliding to each click
with a ripple, keycaps for each shortcut, burned-in captions, the take inset on a brand gradient
with rounded corners and a shadow. Writes OUT.srt beside it. square: 1080x1080, the 30-second cut
(SQUARE_CUT below), each piece framed on its focus region, dissolving into the next. Both get a
silent stereo track, or with --vo=DIR (or LEON_DEMO_VO) a voice-over from DIR's clips: main-NN.mp3,
one a scene (VO_SCENES), and square.mp3; mono voice in both channels, AAC 48 kHz, -16 LUFS. --frames renders only those output frames (for checking a
stretch); --blur turns on motion blur for the camera's moves (off by default: it ghosts terminal
text); --workers splits the frames over N processes (default 4). stills writes single frames at
output times T, as PNGs, to check a look.
"""
import json
import math
import os
import subprocess
import sys
from concurrent.futures import ProcessPoolExecutor
from pathlib import Path

from PIL import Image

from draw import card, ease
from frame import BASE, FPS, MAIN, SQUARE, SRC_H, SRC_W, Renderer, fit, load_cue

# ---------------------------------------------------------------- the timeline

# the brew line only once the first release is out (the cask doesn't exist before): LEON_DEMO_INSTALL
END_LINES = ["a terminal multiplexer for coding agents", "github.com/aindong/leon-cli",
             os.environ.get("LEON_DEMO_INSTALL", "Open source (MIT) · macOS app · CLI for macOS and Linux")]
TITLE, END = 2.5, 6.0
COLD = 3.0  # the cold open: the 50-tab overview, live, so a feed sees motion in the first second
ESC_MARGIN = 0.3  # seconds the cold open ends before the overview's esc

# the 30-second cut: (video start, end, focus region in raw pixels, caption, small line); the times
# are a take's whose video leads the script by 1.5 s, moved to the take's own measured lead. The
# square shows at most 1602 px of the screen's 2848: a wide piece takes its left square (1430 + the
# 12% margin), so the sidebar and the panes' text start at its edge instead of being cut
SQUARE_CUT = [
    (43.4, 47.9, [1000, 260, 1300, 1000], "115 MB/s, and typing keeps up.", ""),
    (50.0, 53.2, [0, 9, 1430, 1430], "50 tabs. Every one live.", ""),
    (81.1, 84.5, [700, 9, 1430, 1000], "Pick a machine from your tailnet.", ""),
    (84.5, 87.5, [0, 0, 1200, 1000], "Its sessions, beside your own.", ""),
    # the banner over the attached terminal and its click; then leon on the agent's question (one
    # frame can't hold both: the banner is at the right edge, the pane's text starts at the sidebar)
    (98.4, 100.2, [1440, 9, 1408, 1100], "It tells you when an agent needs you.", ""),
    (100.4, 102.4, [512, 9, 1430, 1400], "It tells you when an agent needs you.", ""),
    (103.0, 104.1, [0, 9, 1430, 1430], "Quit the app…", ""),
    # from 0.5 s after full screen: the panes are fitted to it again by then
    (107.8, 111.3, [0, 9, 1430, 1430], "…everything is still running.", ""),
    (113.5, 116.0, [0, 9, 1430, 1430], "Keep your work in view.", ""),
]


def pieces(mode, cut, duration, cue=()):
    """the output, as pieces: ("card", kind, dur) or ("take", start, end, fixed_region, caption);
    main's cold open is a take piece whose caption is the string cold"""
    if mode == "main":
        a, b = cut if cut else (duration, duration)
        # from just after the overview opens (its cards fade in) to COLD later, or ESC_MARGIN before
        # its esc if sooner (scene 6's ⌘K follows it at once); a title card when the take has no
        # overview, or one too short to open on
        m = next((r["vt"] for r in cue if r["type"] == "mark" and r["name"] == "5 overview"), None)
        esc = min((r["vt"] for r in cue if r["type"] == "key" and r["combo"] == "esc" and m is not None and r["vt"] > m),
                  default=math.inf)
        if m is None or esc - ESC_MARGIN - (m + 0.4) < 1.0:
            head = ("card", "title", TITLE)
        else:
            head = ("take", m + 0.4, min(m + 0.4 + COLD, esc - ESC_MARGIN), None, "cold")
        return [head, ("take", 0.0, a, None, None), ("take", b, duration, None, None), ("card", "end", END)]
    d = next((r["lead"] for r in cue if r["type"] == "take"), 1.5) - 1.5
    out = [("take", s + d, e + d, r, (c, sm)) for s, e, r, c, sm in SQUARE_CUT]
    return out + [("card", "end", 30.0 - sum(e - s for s, e, *_ in SQUARE_CUT))]


# the voice-over's scenes, one clip a scene (main-NN.mp3, NN the index): each starts VO_LEAD after
# its scene's start: a mark's, or "cold" (the cold open), "take" (the take's first frame, the empty
# state, after the cold open), "end" (the end card); so a line doesn't land on a cut
VO_SCENES = ["cold", "take", "2 build-up", "3 agent runs its tests", "4 activity", "5 speed", "6 switch",
             "7 settings", "8 leon do", "9 tailscale", "10 attach", "10 banner", "11 quit", "12 overview", "end"]
VO_LEAD = 0.3


def vo_windows(mode, cut, duration, cue):
    """each voice-over scene as (start, end) on the output's clock; square: one, up to its end card"""
    ps = pieces(mode, cut, duration, cue)
    lens = [p[2] if p[0] == "card" else p[2] - p[1] for p in ps]
    total = sum(lens)
    if mode != "main":
        return [(0.0, total - lens[-1])]
    a, b = cut if cut else (duration, duration)
    marks = {r["name"]: r["vt"] for r in cue if r["type"] == "mark"}

    def out(vt):  # video time -> output time: after the cold open, less the cut where it's past it
        return lens[0] + (min(vt, a) if vt < b else vt - (b - a))
    at = {"cold": 0.0, "take": lens[0], "end": total - lens[-1]}
    missing = [m for m in VO_SCENES if m not in at and m not in marks]
    if missing:
        sys.exit(f"compose: the voice-over needs every scene; the cue has no mark {', '.join(missing)}")
    starts = [at[m] if m in at else out(marks[m]) for m in VO_SCENES]
    return list(zip(starts, starts[1:] + [total]))


def place(windows, lengths, names=None):
    """{scene: clip seconds} -> [(scene, start, end)]: each clip VO_LEAD into its scene, ending before
    the next scene's clip would start (the last one: before its window ends); one that doesn't fit
    stops the render rather than talk over the next line"""
    placed = []
    for i in sorted(lengths):
        start = windows[i][0] + VO_LEAD
        limit = windows[i + 1][0] + VO_LEAD if i + 1 < len(windows) else windows[i][1]
        if start + lengths[i] > limit:
            sys.exit(f"compose: voice-over clip {(names or {}).get(i, i)} runs {start + lengths[i] - limit:.2f} s past its scene")
        placed.append((i, start, start + lengths[i]))
    return placed


def vo_clips(mode, vo_dir):
    """{scene: path} of the voice-over clips in vo_dir (none without one)"""
    if not vo_dir:
        return {}
    names = [f"main-{i:02}.mp3" for i in range(len(VO_SCENES))] if mode == "main" else ["square.mp3"]
    return {i: Path(vo_dir) / n for i, n in enumerate(names) if (Path(vo_dir) / n).exists()}


LOUDNORM = "loudnorm=I=-16:TP=-1.5:LRA=11"


def voice_track(placed, clips, total, wav):
    """the clips mixed at their places into a wav of the video's length, the mono voice the same in
    both channels (short fades at each clip's edges, no clicks); returns the loudnorm filter for
    it, measured (two-pass, linear) on the stereo, as players measure it"""
    ins, chains = [], []
    for k, (i, s, e) in enumerate(placed):
        ins += ["-i", str(clips[i])]
        chains.append(f"[{k}:a]aformat=sample_rates=48000:channel_layouts=mono,afade=t=in:d=0.01,"
                      f"afade=t=out:st={max(0.0, e - s - 0.04):.3f}:d=0.04,adelay={round(s * 1000)}:all=1[v{k}]")
    mix = "".join(f"[v{k}]" for k in range(len(placed))) + f"amix=inputs={len(placed)}:normalize=0,apad,atrim=0:{total:.3f},pan=stereo|c0=c0|c1=c0"
    subprocess.run(["ffmpeg", "-loglevel", "error", "-y", *ins, "-filter_complex", ";".join(chains + [mix]),
                    "-c:a", "pcm_s16le", str(wav)], check=True)
    err = subprocess.run(["ffmpeg", "-hide_banner", "-i", str(wav), "-af", LOUDNORM + ":print_format=json",
                          "-f", "null", "-"], capture_output=True, text=True, check=True).stderr
    m = json.loads(err[err.rindex("{"):err.rindex("}") + 1])
    return (f"{LOUDNORM}:measured_I={m['input_i']}:measured_TP={m['input_tp']}:measured_LRA={m['input_lra']}"
            f":measured_thresh={m['input_thresh']}:offset={m['target_offset']}:linear=true,aresample=48000")


def audio_args(track, norm=None):
    """ffmpeg's audio input and filter for the finished video: the voice track, or silence"""
    if track is None:
        return ["-f", "lavfi", "-i", "anullsrc=r=48000:cl=stereo"], []
    return ["-i", str(track)], ["-af", norm, "-ar", "48000"]


def seconds(path):
    return float(subprocess.run(["ffprobe", "-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0",
                                 str(path)], capture_output=True, text=True, check=True).stdout)


def frame_plan(ps):
    """one entry per output frame: (piece index, time within the piece)"""
    plan = []
    for i, p in enumerate(ps):
        dur = p[2] if p[0] == "card" else p[2] - p[1]
        n = int(round(dur * FPS))
        plan += [(i, k / FPS) for k in range(n)]
    return plan


def decode(raw, start, dur):
    """raw frames from start for dur seconds, at 60 fps, as PIL images"""
    # -ss after -i: an exact seek. Before -i, a still stretch of this variable-rate recording just
    # after the seek point would shift every later frame earlier (the fps filter starts at the
    # first frame it gets)
    cmd = ["ffmpeg", "-loglevel", "error", "-i", str(raw), "-ss", f"{start:.4f}", "-t", f"{dur:.4f}",
           "-vf", f"fps={FPS}", "-f", "rawvideo", "-pix_fmt", "rgb24", "-"]
    p = subprocess.Popen(cmd, stdout=subprocess.PIPE, bufsize=SRC_W * SRC_H * 3)
    size = SRC_W * SRC_H * 3
    last = None
    try:
        while True:
            buf = p.stdout.read(size)
            if len(buf) < size:
                break
            last = Image.frombuffer("RGB", (SRC_W, SRC_H), buf, "raw", "RGB", 0, 1)
            yield last
        p.wait()
        while last is not None:  # past the clip's last frame: hold it
            yield last
    finally:  # closed early (a piece ends before the clip does): end ffmpeg quietly, not by a broken pipe
        if p.poll() is None:
            p.kill()
        p.stdout.close()
        p.wait()


XFADE = 0.3  # seconds each piece dissolves in from the one before


def soft(mode, i, ps=()):
    """whether piece i of ps dissolves in: every join but the main cut's quit and the cut out of its
    cold open, which are cuts on purpose"""
    cold = bool(ps) and ps[0][-1] == "cold"
    return i > 0 and not (mode == "main" and (i == 2 or (i == 1 and cold)))


def piece_frame(R, L, p, pt, src, out_t):
    """one output frame of piece p at pt seconds into it (src: the raw frame, for a take piece)"""
    if p[0] == "card":
        return card(L, p[1], pt, p[2], END_LINES)
    t = p[1] + pt
    if p[4] == "cold":  # the take as it is: its caption and keycap belong to the story, later
        return R.frame(src, t, out_t, captions=[], keys=False, show_cursor=False)
    if p[3] is not None:  # square: framed on the piece's region, its own caption
        cam = fit(p[3], L.vw / L.vh, max_zoom=BASE[2] / L.vw)
        return R.frame(src, t, pt, fixed=cam, captions=[(0, p[2] - p[1], p[4][0], p[4][1])])
    return R.frame(src, t, out_t)


def last_frame(R, L, raw, p, out_t):
    """piece p's last output frame, for the dissolve into the piece after it"""
    dur = p[2] if p[0] == "card" else p[2] - p[1]
    pt = dur - 1 / FPS
    src = None
    if p[0] == "take":
        # a variable-rate take can have no frame in its last 1/60 s: decode the last 0.2 s and keep
        # the last frame it gives (decode holds the last one past the end)
        frames = decode(raw, max(p[1], p[2] - 0.2), 0.2)
        for _ in range(int(0.2 * FPS)):
            src = next(frames, src)
        if src is None:
            sys.exit(f"compose: no frame of the take near {p[2]:.2f} s")
    return piece_frame(R, L, p, pt, src, out_t)


def render_chunk(args):
    mode, raw, cue_path, f0, f1, cut, duration, out, blur = args
    os.nice(10)
    L = MAIN if mode == "main" else SQUARE
    cue = load_cue(cue_path)
    R = Renderer(L, cue, blur)
    ps = pieces(mode, cut, duration, cue)
    plan = frame_plan(ps)[f0:f1]
    enc = subprocess.Popen(["ffmpeg", "-loglevel", "error", "-y", "-f", "rawvideo", "-pix_fmt", "rgb24",
                            "-s", f"{L.W}x{L.H}", "-r", str(FPS), "-i", "-", "-c:v", "libx264", "-preset", "medium",
                            "-crf", "18", "-maxrate", "12M", "-bufsize", "24M", "-profile:v", "high",
                            "-pix_fmt", "yuv420p", "-g", "120", str(out)], stdin=subprocess.PIPE)
    src_iter, cur_piece, before = None, None, {}
    for fi, (pi, pt) in enumerate(plan):
        p = ps[pi]
        out_t = (f0 + fi) / FPS
        src = None
        if p[0] == "take":
            if cur_piece != pi:
                src_iter, cur_piece = decode(raw, p[1] + pt, p[2] - p[1] - pt + 0.1), pi
            src = next(src_iter)
        img = piece_frame(R, L, p, pt, src, out_t)
        if soft(mode, pi, ps) and pt < XFADE:
            if pi not in before:
                before[pi] = last_frame(R, L, raw, ps[pi - 1], out_t - pt - 1 / FPS)
            img = Image.blend(before[pi], img, ease((pt + 1 / FPS) / (XFADE + 1 / FPS)))
        enc.stdin.write(img.tobytes())
    enc.stdin.close()
    enc.wait()
    return out


def srt(mode, cue_path, cut, duration, out):
    """the captions as an .srt, on the output's clock"""
    def stamp(s):
        ms = int(round(s * 1000))
        return f"{ms // 3600000:02}:{ms // 60000 % 60:02}:{ms // 1000 % 60:02},{ms % 1000:03}"
    rows = []
    if mode == "main":
        a, b = cut if cut else (duration, duration)
        cue = load_cue(cue_path)
        head = pieces(mode, cut, duration, cue)[0]
        head = head[2] if head[0] == "card" else head[2] - head[1]  # the title card or the cold open
        for c in [r for r in cue if r["type"] == "caption"]:
            s, e, line = c["vt"], c["vuntil"], (c["text"], c.get("small", ""))
            if s < a:  # before the cut: shown up to it
                rows.append((head + s, head + min(e, a), *line))
            if e > b:  # after it: shown from it, shifted back by the cut's length
                rows.append((head + max(s, b) - (b - a), head + e - (b - a), *line))
    else:
        t = 0.0
        for s, e, _, c, sm in SQUARE_CUT:
            rows.append((t, t + e - s, c, sm))
            t += e - s
    with open(out, "w") as f:
        for i, (s, e, text, small) in enumerate(rows, 1):
            f.write(f"{i}\n{stamp(s)} --> {stamp(e)}\n{text}\n" + (f"{small}\n" if small else "") + "\n")


def stills(mode, raw, cue_path, outdir, times, cut=None):
    """single output frames as PNGs, at output times (seconds), for checking the look"""
    L = MAIN if mode == "main" else SQUARE
    cue = load_cue(cue_path)
    R = Renderer(L, cue)
    duration = seconds(raw)
    ps = pieces(mode, cut, duration, cue)
    plan = frame_plan(ps)
    Path(outdir).mkdir(exist_ok=True)
    for ot in times:
        pi, pt = plan[min(len(plan) - 1, int(round(ot * FPS)))]
        p = ps[pi]
        src = next(decode(raw, p[1] + pt, 1.0)) if p[0] == "take" else None
        img = piece_frame(R, L, p, pt, src, ot)
        if soft(mode, pi, ps) and pt < XFADE:  # as render_chunk does
            before = last_frame(R, L, raw, ps[pi - 1], ot - pt - 1 / FPS)
            img = Image.blend(before, img, ease((pt + 1 / FPS) / (XFADE + 1 / FPS)))
        img.save(Path(outdir) / f"{mode}-{ot:06.2f}.png")


def main():
    if sys.argv[1] == "stills":  # stills MODE RAW CUE OUTDIR [--cut=A:B] T...
        args = [a for a in sys.argv[6:] if not a.startswith("--")]
        cut = next((tuple(map(float, o.split("=")[1].split(":"))) for o in sys.argv[6:] if o.startswith("--cut=")), None)
        return stills(sys.argv[2], sys.argv[3], sys.argv[4], sys.argv[5], [float(a) for a in args], cut)
    mode, raw, cue_path, out = sys.argv[1:5]
    opts = sys.argv[5:]
    cut = next((tuple(map(float, o.split("=")[1].split(":"))) for o in opts if o.startswith("--cut=")), None)
    frames = next((tuple(map(int, o.split("=")[1].split(":"))) for o in opts if o.startswith("--frames=")), None)
    blur = "--blur" in opts
    workers = int(next((o.split("=")[1] for o in opts if o.startswith("--workers=")), "4"))
    vo_dir = next((o.split("=", 1)[1] for o in opts if o.startswith("--vo=")), os.environ.get("LEON_DEMO_VO"))
    duration = seconds(raw)
    cue = load_cue(cue_path)
    n = len(frame_plan(pieces(mode, cut, duration, cue)))
    f0, f1 = frames if frames else (0, n)
    out = Path(out)
    clips = {} if frames else vo_clips(mode, vo_dir)  # a stretch rendered for a look stays silent
    if clips:  # before any frame is rendered: a clip that doesn't fit stops it now
        windows = vo_windows(mode, cut, duration, cue)
        placed = place(windows, {i: seconds(p) for i, p in clips.items()}, {i: p.name for i, p in clips.items()})
    tmp = out.with_suffix(".parts")
    tmp.mkdir(exist_ok=True)
    (tmp / "voice.wav").unlink(missing_ok=True)  # left by a render that stopped
    step = math.ceil((f1 - f0) / workers)
    jobs = [(mode, raw, cue_path, a, min(a + step, f1), cut, duration, tmp / f"part{i:02}.mp4", blur)
            for i, a in enumerate(range(f0, f1, step))]
    with ProcessPoolExecutor(len(jobs)) as pool:
        parts = list(pool.map(render_chunk, jobs))
    lst = tmp / "parts.txt"
    lst.write_text("".join(f"file '{p.name}'\n" for p in parts))
    track, norm = None, None
    if clips:
        for i, s, e in placed:
            w = windows[i]
            print(f"vo: {clips[i].name} {s:7.2f}-{e:7.2f} in scene {w[0]:7.2f}-{w[1]:7.2f}")
        track = tmp / "voice.wav"
        norm = voice_track(placed, clips, n / FPS, track)
    ain, af = audio_args(track, norm)
    subprocess.run(["ffmpeg", "-loglevel", "error", "-y", "-f", "concat", "-safe", "0", "-i", str(lst), *ain,
                    "-map", "0:v", "-map", "1:a", *af, "-c:v", "copy", "-c:a", "aac", "-b:a", "128k", "-shortest",
                    "-movflags", "+faststart", str(out)], check=True)
    for p in parts + [lst] + ([track] if track else []):
        p.unlink()
    tmp.rmdir()
    if not frames:
        srt(mode, cue_path, cut, duration, out.with_suffix(".srt"))
    print(f"compose: {out} ({f1 - f0} frames)")


if __name__ == "__main__":
    main()
