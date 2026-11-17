"""cut.py IN OUT [BEATS] | cut.py --print IN [BEATS]: the take without the reopen's windowed second.

--print only prints the cut as A:B (seconds of IN), for compose.py --cut.

After ⌘Q the desktop shows (bright wallpaper), then the app opens windowed over it and goes full
screen. The edit keeps the first second of desktop ("the app is gone") and cuts straight to the
first full-screen frame after it. Frames are told apart by their mean luma: the dark app full
screen is about 44, windowed over the wallpaper about 64, the desktop about 104. The run used is
the first bright one after the "11 quit" beat in BEATS (take-beats.txt), so the light-theme moment
earlier is never taken for it.
"""
import pathlib
import re
import subprocess
import sys

args = sys.argv[1:]
only_print = args[:1] == ["--print"]
if only_print:
    args = args[1:]
    args.insert(1, None)
src, out = args[0], args[1]
beats = pathlib.Path(args[2] if len(args) > 2 else pathlib.Path(src).with_name("take-beats.txt"))
LEAD = 1.5  # the video starts this long before the script's clock (run-demo.sh's `sleep 1.5`)
quit_at = next((float(line.split()[0]) + LEAD for line in beats.read_text().splitlines() if "11 quit" in line), None)
if quit_at is None:
    sys.exit(f"cut.py: no '11 quit' beat in {beats}")

# the path goes in as an input file, not into filter text, so any characters in it are fine
log = subprocess.run(
    ["ffmpeg", "-hide_banner", "-i", src, "-vf", "signalstats,metadata=print:key=lavfi.signalstats.YAVG:file=-",
     "-f", "null", "-"], capture_output=True, text=True, check=True).stdout
frames, t = [], None
for line in log.splitlines():
    if m := re.search(r"pts_time:([\d.]+)", line):
        t = float(m.group(1))
    elif m := re.search(r"YAVG=([\d.]+)", line):
        frames.append((t, float(m.group(1))))
BRIGHT = 55
after = [(t, y) for t, y in frames if t >= quit_at - 0.5]
first = next((t for t, y in after if y > BRIGHT), None)
if first is None:
    sys.exit(f"cut.py: no desktop frames after the quit at {quit_at:.1f} s")
last = first
for t, y in after:
    if t < first:
        continue
    if y <= BRIGHT:
        break
    last = t
keep_until = first + 1.0
# 0.35 s more: the first dark frames are macOS's full-screen animation, the window still see-through
resume = next((t for t, y in frames if t > last and y <= BRIGHT), None)
resume = resume + 0.35 if resume is not None else None
if resume is None or resume <= keep_until:
    sys.exit("cut.py: the desktop is too short to cut")
if only_print:
    print(f"{keep_until:.2f}:{resume:.2f}")
    sys.exit(0)
print(f"cut.py: desktop {first:.2f}-{last:.2f} s; keeping to {keep_until:.2f}, resuming at {resume:.2f}")
graph = (f"[0:v]trim=0:{keep_until},setpts=PTS-STARTPTS[a];"
         f"[0:v]trim=start={resume},setpts=PTS-STARTPTS[b];[a][b]concat=n=2:v=1[v]")
subprocess.run(["ffmpeg", "-loglevel", "error", "-y", "-i", src, "-filter_complex", graph, "-map", "[v]",
                "-c:v", "libx264", "-crf", "16", "-pix_fmt", "yuv420p", out], check=True)
