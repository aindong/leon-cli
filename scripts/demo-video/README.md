# The launch video kit

Everything that makes leon's launch video: a scripted take in a macOS VM, with real example
projects and scripted coding agents, and a compositor that turns the raw take into the finished
video (zooms onto what matters, a drawn pointer with click ripples, keycaps for every shortcut,
captions, a brand frame, title and end cards). A take is repeatable: the same keys at the same
moments every time, driven over ssh. Nothing touches the host's own leon, screen or sessions.

| Path | What it is |
|---|---|
| `run-demo.sh` | On the host: `boot`, `push`, `setup`, `take`, `cue`, `render`, `retake`, `shot`, `check`, `stop`. The scenes and their timing are its `scene1`…`scene12` functions. |
| `kit/guest.sh` | In the VM, as `demo`: reset, stage, start the app, the take's beats. |
| `kit/bin/agent`, `kit/scenes/` | The scripted agent: reads a scene file, prints at a reading pace, reports its state with OSC 7501, asks its question and waits, and drives leon the way an agent with leon's skill does (`leon split`, `leon wait`, `leon capture`). No AI service runs. |
| `kit/projects/` | Three small Python projects (stdlib only) whose tests really run; `.next/N/` is each agent's change, applied during the take. |
| `kit/bin/tailscale` | A stand-in found through `LEON_TAILSCALE`: answers only `status --json`, with a made-up tailnet. |
| `kit/bin/mklog.py`, `kit/bin/ticker` | The 400 MB coloured log for the speed scene, and the background jobs of the 50-tab overview. |
| `kit/init.lua`, `kit/zshrc` | The take's leon config (font, theme, no banners until scene 10 turns one on, the `review` action, `docs/lua/keys.md`'s ctrl+alt+g git popup) and a neutral prompt. |
| `keypost.swift` | Posts real key codes and clicks to the focused app in the VM, and keeps the parked pointer stepping during a take; `push` builds it. |
| `vnc.py` | Screenshots of the headless VM over its VNC server. |
| `cut.py` | Finds the second where the app reopens windowed after ⌘Q, so the edit can drop it. |
| `lead.py` | Measures how far the raw take runs ahead of the script's clock, into the cue file. |
| `compose/` | The compositor: `compose.py` (timeline, render, CLI), `frame.py` (layouts, camera, one frame), `draw.py` (gradient, keycaps, pointer, wordmark, cards), `test_compose.py`, `requirements.txt`. |
| `frames.sh` | Contact sheets of a video at given seconds, for checking a take or a render frame by frame. |

Everything a take or a render writes goes to `$DEMO_WORK`, by default `target/demo-video/`,
which git ignores: the VM's ssh key and address, the raw take, its beats and cue file, the
renders, the compositor's venv.

## What it needs

- An Apple Silicon Mac with Tart, the macOS VM tool (its own licence; read it before use),
  and a macOS VM for it, here named `leon-demo` (`DEMO_VM` to change), 6 cores, 1440×810 pt at 2×
  (2880×1620 px). `TART_HOME`, if set, says where tart keeps it.
- `ffmpeg` with libx264, Python 3.12 or newer, and `swiftc` (Xcode's command line tools).
- A release build of leon's app: `scripts/bundle.sh` (it writes `target/release/Leon.app`).

## One-time VM setup

These need a person once, over VNC (`tart run leon-demo --vnc-experimental` prints the address):

1. Create the user `demo`, finish Setup Assistant without an Apple Account, turn on auto-login
   for `demo`, Remote Login (ssh) and no screen lock.
2. In System Settings › Privacy & Security, give `/usr/libexec/sshd-keygen-wrapper` Screen &
   System Audio Recording and Accessibility: processes started over ssh then may record the
   screen and post keys. Allow the first capture's one-time prompt.
3. Put a standalone Python 3.12+ in `~demo/python` (the VM has no developer tools, and the stub
   `python3` asks to install them).
4. An ssh key for the host: `mkdir -p target/demo-video && ssh-keygen -t ed25519 -N "" -f target/demo-video/key`, and the
   `.pub` in `~demo/.ssh/authorized_keys`.
5. The "devbox": the VM's second user (here `admin`) plays another machine, reached over real
   ssh. As `demo`: `ssh-keygen -t ed25519 -N "" -f ~/.ssh/devbox`; the `.pub` in that user's
   `authorized_keys`; this in `~demo/.ssh/config`:

   ```text
   Host devbox
     HostName 127.0.0.1
     User admin
     IdentityFile ~/.ssh/devbox
   ```

   and a copy of `~demo/python` in that user's home. `run-demo.sh push` copies leon there.
6. Banners, for the attach scene: in System Settings › Notifications, set "when mirroring or
   sharing the display" to Allow Notifications (macOS counts the VM's display as shared and hides
   every banner otherwise), and after the first `push` and `setup`, turn on Leon's "Allow
   notifications" there (its first request comes while banners are hidden, so macOS records it as
   denied). Check it after a `push` of a new build: `usernoted` in the VM's log says
   `authorizationStatus: Denied` when it is off again.

`guest.sh prefs` (run by `setup`) does the rest each time, safely: it hides the desktop widgets,
turns off macOS's tips (their "See what's new" banner stays on screen until clicked), gives
Terminal a "Demo" profile (its Clear Dark, opaque, 16 pt, no "Last login" line) for the attach scene, maps the made-up tailnet's `devbox.example-tailnet.ts.net` to the same account, and makes the
400 MB log once.

## Making a take

```sh
scripts/bundle.sh
scripts/demo-video/run-demo.sh boot    # headless, waits for ssh
scripts/demo-video/run-demo.sh push    # Leon.app, the kit and keypost into the VM; leon onto the devbox
scripts/demo-video/run-demo.sh check   # the projects' tests, before and after each agent's change
scripts/demo-video/run-demo.sh setup   # clean: the empty state, the devbox staged, a terminal on the desktop, full screen
scripts/demo-video/run-demo.sh take    # scenes 1-12, about 117 s; take 3-5 films only those
scripts/demo-video/run-demo.sh stop
```

After a change lands, one command films it again: `run-demo.sh retake REV` (with the VM booted)
builds Leon.app from a clean export of commit `REV` on this Mac (the VM has no Rust toolchain;
the build has its own target dir in `$DEMO_WORK/cargo`, so the next retake builds only what
changed), then runs `push`, `setup`, `take` and `render`. `push` takes any other build from
`DEMO_APP=path/to/Leon.app`. It also copies this Mac's `git` (Apple's, which needs only system
libraries) into the VM's kit, for the popup's `git log`: the VM has no developer tools.

`setup` starts from nothing every time: it quits the app, stops both leon servers and forgets
their sessions (the take's own `LEON_RUNTIME_DIR` and `LEON_STATE_DIR` in the VM, never anything
else), and copies the projects fresh. `take` writes `take-raw.mov` (2880×1620, variable frame
rate), `take.mp4` (1080p60, for a look), `take-beats.txt` and `take-cue.jsonl`. `shot NAME` saves
a VNC screenshot to `shots/`.

Things learnt the hard way: `screencapture` started in the background inside the VM hangs, so
it runs in the foreground of its own ssh session; `screencapture -V` stops only on a frame after
its time, so a take ends on panes that still print; after a pane closes, one still second ends
`screencapture -v` early (with any leon build), so during a take the parked pointer steps 4 pt up
and down the right edge, inside the strip the render crops off (`keypost wiggle`); ⌘K matches anywhere in a name, so the take
types whole words; the pointer is parked on the right edge, inside what the render crops; the
recorder's own start varies (from 0 to 1.5 s after it is asked, on 2026-11-17), so `take` measures
the lead on the video (`lead.py`: the first frame that changes is scene 2's ⌘N) instead of
trusting the 1.5 s it waits; one take in four stopped at 64 s with `screencapture: exit 0`, so
check `take-raw.mov`'s length.

## Rendering

```sh
scripts/demo-video/run-demo.sh render
```

That makes a venv in `target/demo-video/venv` from `compose/requirements.txt` once, finds the
reopen's windowed second with `cut.py --print`, and renders, at `nice -n 10` with 4 processes:

- `renders/leon-demo-main.mp4`: 16:9, 1920×1080, 60 fps: a 3 s cold open (the 50-tab overview,
  live, from the take's `5 overview` beat, without its caption or keycap), a cut to the empty
  state and the take from there, end card;
- `renders/leon-demo-square-30s.mp4`: 1:1, 1080×1080, 60 fps: the 30-second cut
  (`SQUARE_CUT` in `compose.py`), its pieces dissolving into each other;
- an `.srt` beside each, and a silent stereo track in each, for a voice-over.

H.264 High, CRF 18 capped at 12 Mb/s, 2 s GOPs, `+faststart`: X and LinkedIn take it as it is.
On a 12-core Mac with other work running, the main cut takes 2 to 3 minutes, the square 1.
`compose.py stills MODE RAW CUE DIR T…` renders single frames to check a look;
`frames.sh VIDEO DIR T…` makes contact sheets of a render. `LEON_DEMO_INSTALL` sets the end card's
last line (the install command, once a release exists). Tests:
`target/demo-video/venv/bin/python -m unittest discover -s scripts/demo-video/compose`.

What it draws, and the limits it keeps:

- **Zooms** ease in and out over 0.7 s (smootherstep: speed and acceleration start and end at
  zero), with the width moving in log space; beats closer than 0.7 s chain straight from one
  region to the next. A zoom never goes past one raw pixel per output pixel, so text stays sharp:
  **1.84× in 16:9, 2.89× in 1:1**. The camera shows the cue's region widened by 12% about its
  centre and to the frame's shape; choose regions whose widened edges fall on pane borders.
- **The pointer** is drawn (an original shape): it glides to each click on a gentle arc and
  leaves an amber ripple. The VM's own pointer never shows.
- **Keycaps** for every shortcut and for ↩/esc where they matter, in macOS order (⌃ ⌥ ⇧ ⌘), the
  symbols drawn as strokes; typed text is never shown key by key.
- **Captions** at the top of the frame, keycaps at the bottom: neither covers the take.
- **Fonts and art**: Instrument Sans, read from `brand/fonts/instrument-sans-latin.woff2` (SIL
  Open Font License 1.1, `brand/fonts/OFL.txt`); the app icon `brand/icon/leon-1024.png`; the
  wordmark drawn from `brand/wordmark-dark.svg`. Nothing else is bundled or downloaded.
- Motion blur exists (`--blur`) but is off: blending sub-frames ghosts terminal text.

## The cue file

`take-cue.jsonl`, one JSON object a line, written by `run-demo.sh` as the take sends each step.
`t` is seconds on the script's clock; the video runs `lead` seconds ahead of it.

| `type` | Fields | Meaning |
|---|---|---|
| `take` | `lead`, `width`, `height` | first line: the raw video's size and lead (measured by `lead.py`) |
| `mark` | `name` | a scene or beat starts (also in `take-beats.txt`) |
| `key` | `combo` (`cmd+shift+\`), `show` | a key sent; `show` puts it on screen as keycaps |
| `type` | `text` | text typed (never shown as keycaps) |
| `click` | `x`, `y` | a click, in raw pixels |
| `step` | `step` | a `guest.sh` step (`go shelf-tests`, `build 2`, …) |
| `zoom` | `region` `[x, y, w, h]` raw pixels, `until` | zoom onto the region until `until` |
| `caption` | `text`, `until`, `small` | a caption (and an `.srt` line), with an optional smaller line |

`run-demo.sh cue [BEATS]` (default: the last take's `take-beats.txt`) rebuilds the cue file of an earlier take without the VM: the
scenes run on a virtual clock, re-anchored at each mark to the take's real beat times. Change a
zoom or a caption in `run-demo.sh`, run `cue`, then `render`.

## Where the numbers in the captions come from

Only leon's own measured numbers, never a comparison with another terminal. Each comes from a
script in `scripts/perf/`; run it on the release build that is filmed, on the same Mac, and
change the caption in `run-demo.sh` (and the storyboard) if the number moved.

| Caption | Measured by | Recorded in |
|---|---|---|
| 115 MB/s through the terminal | `scripts/perf/flood.sh` (a 100 MB `cat` of text and colour) | docs/performance-cat.md, "Before and after" |
| 60 frames a second, none missed | `scripts/perf/scale.sh` (100 panes printing) and `flood.sh` (frames shown) | docs/performance.md, "The app end to end" |
| 50 tabs, every card live in 33 ms | `scripts/perf/overview.sh` | docs/performance.md, "The overview with 50 tabs" |
| 159 ms from launch to prompt | `scripts/perf/startup.sh` | docs/performance.md, "The app end to end" |
| Under 1% of a core when idle | `scripts/perf/idle-cpu.sh` | docs/performance.md, "The app end to end" |
| Typing keeps up | no number by default | if one is wanted, only "a key reaches the painted frame in 12 ms" (`keylat.sh`, in-app; the outside clock measures about 50 ms to pixels on screen, so never call 12 ms key-to-screen latency) |

The speed is measured on the host, not in the VM: the VM's display path adds 20 to 30 ms to
input and lowers its capture rate.

## What is staged

- The agents are scripts: their words are written in `kit/scenes/`, but the edits they make, the
  tests they run in the panes they open, and the questions they wait on are real.
- The tailnet is made up: `kit/bin/tailscale` answers `status --json` with `example-tailnet.ts.net`
  and `demo@example.com`, and the devbox is the VM's second account over real ssh. Captions say
  "pick a machine from your tailnet", never that no keys are needed.
- Speed numbers are captions from the host's measurements (above), not filmed in the VM.
- status-page's git history (the popup's `git log --graph`) is made up at `reset`: one real
  commit of the project, then empty commits and a merged branch with fixed dates.
- The banner in scene 10 is leon's own (the app posts it when logtally's agent asks its second
  question); only when the agent asks is scripted. leon is full screen in its own Space, so the
  terminal is on the desktop and the shot shows one or the other, never both at once.

## Licences of what it uses

| Tool | Licence |
|---|---|
| ffmpeg with libx264 | LGPL 2.1+ / GPL 2+ (x264) |
| numpy (`requirements.txt`) | BSD-3-Clause |
| Pillow (`requirements.txt`) | MIT-CMU (HPND) |
| Instrument Sans (in `brand/fonts/`) | SIL Open Font License 1.1 |
| Tart | its own licence: read it before use |
