"""lead.py RAW CUE: measure how far the raw take runs ahead of the script's clock, and write it into
the cue file's first line (`lead`); prints it.

`run-demo.sh take` starts the recorder, waits 1.5 s and starts the clock, but the recorder's own
start varies (0 to 1.5 s seen in one VM, one evening): a fixed lead puts every zoom, caption and
click up to 1.5 s off. The screen is still until the take's first shortcut with a modifier (the launch video's ⌘N opens
a session; the web demo's ⌘, opens Settings, its typing and Return before it are too small a change to count), so
the first frame that changes is that shortcut's:
lead = when it shows in the video - when the cue sent it.
"""
import json
import subprocess
import sys

W, H = 178, 100  # small and grey is enough to see a session open
STILL = 2.0  # mean grey difference per pixel below which two frames are the same screen


def first_change(raw, until=12.0, fps=30):
    """seconds into raw of the first frame that differs from the one before it (the right 32 px,
    where the parked pointer steps, left out), or None"""
    out = subprocess.run(["ffmpeg", "-loglevel", "error", "-i", raw, "-t", str(until), "-vf",
                          f"fps={fps},crop=2848:1602:0:9,scale={W}:{H},format=gray", "-f", "rawvideo", "-"],
                         capture_output=True, check=True).stdout
    frames = [out[i:i + W * H] for i in range(0, len(out) - W * H + 1, W * H)]
    for n in range(1, len(frames)):
        if sum(abs(a - b) for a, b in zip(frames[n - 1], frames[n])) / (W * H) > STILL:
            return n / fps
    return None


def main():
    raw, cue_path = sys.argv[1:3]
    rows = [json.loads(line) for line in open(cue_path) if line.strip()]
    sent = next(r["t"] for r in rows if r["type"] == "key" and "+" in r["combo"])
    seen = first_change(raw)
    if seen is None:
        sys.exit(f"lead.py: nothing changes in the first 12 s of {raw}")
    lead = round(seen - sent, 3)
    if not -0.5 <= lead <= 3.0:  # not that key's frame: something else moved; keep the cue as it is
        sys.exit(f"lead.py: a lead of {lead} s is not believable (first change at {seen:.2f} s)")
    rows[0]["lead"] = lead
    with open(cue_path, "w") as f:
        f.write("".join(json.dumps(r) + "\n" for r in rows))
    print(lead)


if __name__ == "__main__":
    main()
