"""compose.py: a raw take + its cue file -> the finished video.

  compose.py main   RAW CUE OUT.mp4 [--cut=A:B] [--frames=F0:F1] [--workers=N] [--blur]
  compose.py square RAW CUE OUT.mp4 [--frames=F0:F1] [--workers=N]
  compose.py stills MODE RAW CUE OUTDIR [--cut=A:B] T...

main: 16:9 1920x1080 at 60 fps: a title card, the take (with the stretch A..B of video time cut
out), an end card; eased zooms onto the cue's zoom beats, a drawn pointer gliding to each click
with a ripple, keycaps for each shortcut, burned-in captions, the take inset on a brand gradient
with rounded corners and a shadow. Writes OUT.srt beside it. square: 1080x1080, the 30-second cut
(SQUARE_CUT below), each piece framed on its focus region, dissolving into the next. Both get a
silent stereo track for the voice-over. --frames renders only those output frames (for checking a
stretch); --blur turns on motion blur for the camera's moves (off by default: it ghosts terminal
text); --workers splits the frames over N processes (default 4). stills writes single frames at
output times T, as PNGs, to check a look.
"""
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

# the 30-second cut: (video start, end, focus region in raw pixels, caption, small line)
SQUARE_CUT = [
    (43.4, 47.9, [1000, 260, 1300, 1000], "115 MB/s, and typing keeps up.", ""),
    (50.0, 53.2, [0, 0, 1700, 1400], "50 tabs. Every one live.", ""),
    (57.2, 60.2, [734, 167, 1200, 1290], "Your git history on one key.", ""),  # the popup, then hidden
    (80.3, 84.5, [880, 100, 1120, 360], "Pick a machine from your tailnet.", ""),
    (84.5, 87.5, [0, 0, 1200, 1000], "Its sessions, beside your own.", ""),
    (92.5, 93.6, [0, 9, 2848, 1602], "Quit the app…", ""),
    (96.8, 100.3, [0, 9, 2848, 1602], "…everything keeps running.", ""),
    (103.0, 105.5, [0, 0, 2000, 1560], "Keep your work in view.", ""),
]


def pieces(mode, cut, duration):
    """the output, as pieces: ("card", kind, dur) or ("take", start, end, fixed_region, caption)"""
    if mode == "main":
        a, b = cut if cut else (duration, duration)
        return [("card", "title", TITLE), ("take", 0.0, a, None, None), ("take", b, duration, None, None),
                ("card", "end", END)]
    out = [("take", s, e, r, (c, sm)) for s, e, r, c, sm in SQUARE_CUT]
    return out + [("card", "end", 30.0 - sum(e - s for s, e, *_ in SQUARE_CUT))]


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


def soft(mode, i):
    """whether piece i dissolves in: every join but the main cut's quit, which is a cut on purpose"""
    return i > 0 and not (mode == "main" and i == 2)


def piece_frame(R, L, p, pt, src, out_t):
    """one output frame of piece p at pt seconds into it (src: the raw frame, for a take piece)"""
    if p[0] == "card":
        return card(L, p[1], pt, p[2], END_LINES)
    t = p[1] + pt
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
    ps = pieces(mode, cut, duration)
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
        if soft(mode, pi) and pt < XFADE:
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
        for c in [r for r in load_cue(cue_path) if r["type"] == "caption"]:
            s, e, line = c["vt"], c["vuntil"], (c["text"], c.get("small", ""))
            if s < a:  # before the cut: shown up to it
                rows.append((TITLE + s, TITLE + min(e, a), *line))
            if e > b:  # after it: shown from it, shifted back by the cut's length
                rows.append((TITLE + max(s, b) - (b - a), TITLE + e - (b - a), *line))
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
    R = Renderer(L, load_cue(cue_path))
    duration = float(subprocess.run(["ffprobe", "-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0",
                                     raw], capture_output=True, text=True, check=True).stdout)
    ps = pieces(mode, cut, duration)
    plan = frame_plan(ps)
    Path(outdir).mkdir(exist_ok=True)
    for ot in times:
        pi, pt = plan[min(len(plan) - 1, int(round(ot * FPS)))]
        p = ps[pi]
        src = next(decode(raw, p[1] + pt, 1.0)) if p[0] == "take" else None
        img = piece_frame(R, L, p, pt, src, ot)
        if soft(mode, pi) and pt < XFADE:  # as render_chunk does
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
    duration = float(subprocess.run(["ffprobe", "-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0",
                                     raw], capture_output=True, text=True, check=True).stdout)
    n = len(frame_plan(pieces(mode, cut, duration)))
    f0, f1 = frames if frames else (0, n)
    out = Path(out)
    tmp = out.with_suffix(".parts")
    tmp.mkdir(exist_ok=True)
    step = math.ceil((f1 - f0) / workers)
    jobs = [(mode, raw, cue_path, a, min(a + step, f1), cut, duration, tmp / f"part{i:02}.mp4", blur)
            for i, a in enumerate(range(f0, f1, step))]
    with ProcessPoolExecutor(len(jobs)) as pool:
        parts = list(pool.map(render_chunk, jobs))
    lst = tmp / "parts.txt"
    lst.write_text("".join(f"file '{p.name}'\n" for p in parts))
    subprocess.run(["ffmpeg", "-loglevel", "error", "-y", "-f", "concat", "-safe", "0", "-i", str(lst),
                    "-f", "lavfi", "-i", "anullsrc=r=48000:cl=stereo", "-map", "0:v", "-map", "1:a",
                    "-c:v", "copy", "-c:a", "aac", "-b:a", "128k", "-shortest", "-movflags", "+faststart",
                    str(out)], check=True)
    for p in parts:
        p.unlink()
    lst.unlink()
    tmp.rmdir()
    if not frames:
        srt(mode, cue_path, cut, duration, out.with_suffix(".srt"))
    print(f"compose: {out} ({f1 - f0} frames)")


if __name__ == "__main__":
    main()
