# Performance: the app end to end

The app itself, measured from outside and inside: key to pixel, frames, start-up, memory, idle
CPU, scale, a 2-hour agent soak, and the overview with 50 tabs. The speed bench and the machine
are in docs/performance.md; the charts' data is in [docs/perf/](perf/).

## The app end to end (scripts/perf)

The bench stops at the app's screen model. `scripts/perf/` measures the app itself, under load,
with one command each; every run starts its own release leon with private `LEON_RUNTIME_DIR`,
`LEON_STATE_DIR`, `LEON_CONFIG` and `HOME` (`/bin/sh`, whose prompt is a coloured bar), waits for
the 1-minute load to fall under 3 before each step, and notes the load beside each number in
`.agent/perf/<run>/`. Two clocks, both on the boot clock (`CLOCK_UPTIME_RAW`):

- **In the app**: `LEON_FRAME_LOG=PATH` (off unless set) logs each frame (render start, last pane
  painted, the next display tick) and each key: AppKit's stamp on the event, the key sent to the pane, the first
  paint of the focused pane whose cursor moved, and the next display tick. That tick is when GPUI
  can next present, not when the frame was shown.
- **From outside**: `winframes.swift` captures one window with ScreenCaptureKit and hashes each
  frame, so only changed frames count. It posts synthetic keys to a pid and times each to the
  first frame whose echo bar (`echo.py` flips it red and green per key) has that key's colour:
  key posted to frame composited, which is all of the app's path plus the window server's. Not
  seen: scan-out. The floor is a window of the tool's own that repaints the moment a key arrives.
  `external.sh` runs these outside measurements on any terminal given as a launch command.

Apple M6 (12 cores), macOS 27, a 60 Hz display, release build, load 2 to 4.5 (a remote-desktop
session was streaming the screen, which slows every outside number the same way):

| Measurement | Command | Result |
|---|---|---|
| key to pixel, idle / one pane flooding / eight panes busy | `keylat.sh` | in app, painted: p50 11.9 / 12.4 / 14.9 ms, p99 20.8 / 20.4 / 25.1 ms; outside: p50 50.3 / 49.7 / 51.9 ms, p99 66.6 / 65.1 / 71.8 ms; floor p50 18.8 ms |
| a 50 ms echo delay | `keylat.sh --loads idle --delay 50` | in app +54 ms, outside +48 ms: both clocks see it |
| `cat` 100 MB of text and colour, 1 / 8 panes | `flood.sh` | 55.5 / 57.3 frames a second shown (the capture's own floor 54.7 / 55.6), none missed; frame build p50 1.0 / 1.4 ms; screen current 46 ms after the last byte |
| start-up to the prompt drawn, no server / server up | `startup.sh` | 159 / 154 ms (window on screen 188 / 190 ms); `leon run --wait -- true` 13 ms; `leon server start` 32 ms |
| memory (footprint) | `memory.sh` | server 7.3 MiB idle, +0.53 MiB a pane, +20.7 MiB per 10,000 lines of scrollback in an 80-column pane; app 137 MiB, +0.06 MiB a pane; a 1 GB flood peaks the app at 163 MiB and it is back to 135 MiB within 30 s; a font change with 9 hidden 10,000-line panes adds 0.1 MiB to the server (387 to 414 MiB when hidden panes were refit too) |
| CPU with 12 idle panes: still / one pane working / app hidden | `idle-cpu.sh` | server under 0.1% of a core in each; app 2.3% / 6.6% / 1.9%, 80 / 58 / 32 wake-ups a second at first; since the poll runs only when woken, 0.74% / 4.4% / 0.06%, 18 / 18 / 0.3 (load 1.4 to 2.3) |
| 25 and 100 panes, 50 sessions, 10 tabs | `scale.sh` | 100 panes made in 1.4 s, the 100th split as fast as the first (9 ms, the CLI's own start included); 60 frames a second with all 100 printing, frame build p50 5.1 ms; server 88 MiB; a session 11 ms |
| 2-hour agent soak: 12 agents in 4 sessions, a session made and killed and an attach client every minute | `soak.sh 120` | app 162 to 180 MiB throughout (+1.2 MiB/h over the last hour); server 354 to 432 MiB, then 667 to 896 MiB after the 9 hidden panes were fitted to the window's width at minute 48 (their 10,000-line histories now 213 columns; likely a font change, which fitted hidden panes then and now fits only the panes on screen); its live heap was the same at minutes 60 and 120 (748 MB, +448 bytes), so the footprint's sawtooth is freed memory the allocator keeps; `leaks` found none in the server and 56 small objects (2.5 KB) in the app, all leaked by GPUI (docs/performance-memory.md) |

![Key to pixel: the app paints a key's echo 12 to 15 ms after the key at p50, idle or busy, mostly waiting for the next display tick; a screen capture sees it about 50 ms after the key](images/perf/key-to-pixel.svg)

![The app's CPU with 12 idle panes: 0.74% of a core when still and 0.06% when hidden, down from 2.3% and 1.9% once it stopped polling every 8 ms](images/perf/idle-cpu.svg)

![Frame build at p50: 1.0 ms with one pane, 1.4 ms with 8 and 5.1 ms with 100 printing, all inside one 60 Hz frame of 16.7 ms](images/perf/frame-build.svg)

Where the time goes, from `sample`: the app used to poll its model every 8 ms (`POLL`) even when
nothing changed or it was hidden; now its main thread sleeps until woken, and what an idle shown
window still costs is GPUI's display link, which ticks at 60 Hz while the window is visible. A
pulsing status indicator re-lays out the whole window on each of its frames. During a flood the server's parse thread spent more time in leon's own passes over
the bytes (the slice scans in `terminal_view::feed`, the X10 mouse scanner, the OSC 7501 scanner)
than in alacritty's parser (fixed since: docs/performance-cat.md). A keystroke's echo is painted 12 ms after the key (most of it waiting for the next
display tick), then shown about two display frames later: GPUI draws on the display link and
presents through a three-drawable Metal layer.

## Key echo at once

Since 2026-11-20 the app shows a key's echo when the pane's screen arrives, not at GPUI's next
display tick: it asks AppKit to display GPUI's view, and GPUI draws and presents that frame in
the same run-loop pass (`view_echo.rs`). GPUI 0.2.2 leaked a display link each time; leon builds
it with one fix ([architecture/gpui-patch.md](architecture/gpui-patch.md)), and
`crates/leon-app/tests/gpui_display_link.rs` counts the links: 500 shows at once leave 1 link
with the fix, and 109 without it.

The first measurement (#21's probe on the host, crates.io GPUI, 200 keys, `keylat.sh --loads
idle`): key to frame composited from outside p50 49.8 to 36.3 ms, p99 67.3 to 55.0 ms; in the
app, key to echo painted p50 11.1 to 6.8 ms; `flood.sh` unchanged (the display link's pace).
At 120 Hz the saving is about half.

## The overview with 50 tabs

`scripts/perf/overview.sh [--runs N] [--tabs N]` makes 50 tabs in 10 sessions, opens the
overview (⌘⇧\) through System Events and reads `LEON_FRAME_LOG`'s `overview` and
`overview-filled` lines: from the key's AppKit stamp to the display tick after the first overview
frame, and after the first frame in which every card has its tab's screen (the server pushes each
tab's screen once the overview watches it). It fails when the filled median reaches 100 ms. Two
runs of five opens on the machine above, at a load of 10.5 and 8.7 (an upper bound): first frame
p50 16.1 / 17.1 ms, every card filled p50 32.5 / 33.8 ms (max 46.2 ms); frames while open take
1 to 3 ms to build (max 7.5 ms), since each thumbnail is a cached view that draws again only when
its screen changes.

## GPUI phases 5 to 7: before and after

[How leon uses GPUI](architecture/gpui.md) planned a refactor in phases; phases 5 to 7 moved the
model into entities with queued server calls (R4, R8), gave each overlay its own focus (R3, R9)
and sent text through an input handler (R9). Their cost or gain, as an A/B: `2e773bd` (dev just
before phase 5) against `ae96f39` (after phase 7), the same scripts, alternating runs, in a Tart
macOS VM with 6 cores on the Apple M6 (2026-11-20). The host was busy with other work all day
(1-minute load 5 to 30), so the GUI runs went to the VM; the VM's own load was 1 to 4.4 for
`keylat.sh` and `idle-cpu.sh` (beside each number in `.agent/perf/`). The VM's absolute numbers
are not the host's (idle key to paint 26 ms against the host's 12): compare the two columns only.
Medians of three runs (`keylat.sh`) or both of two (the rest), the range after:

| Measurement | before (`2e773bd`) | after (`ae96f39`) |
|---|---|---|
| key to echo painted, in the app, p50: idle / one pane flooding / eight busy | 26.0 / 17.5 / 18.0 ms (25.5–26.6, 14.1–18.7, 17.8–18.7) | 28.3 / 20.8 / 18.0 ms (24.5–41.6, 17.5–26.3, 17.7–19.3) |
| the same at p99 | 51.0 / 52.0 / 52.4 ms | 56.8 / 50.7 / 37.9 ms |
| key to frame composited, outside, p50 | 96.1 / 103.7 / 98.7 ms (floor 20.8–31.9) | 99.9 / 88.0 / 81.3 ms (floor 22.5–32.4) |
| key event to the key sent to the pane, p50 | 9.2 / 12.0 / 11.2 ms | 8.6 / 11.3 / 10.0 ms |
| the app's CPU with 12 idle panes: still / one working / hidden | 1.50, 1.53 / 4.21, 4.72 / 0.08, 0.07% of a core | 0.82, 1.02 / 4.86, 2.07 / 0.02, 0.00% |
| the app's wake-ups a second: still / hidden | 55.7, 55.9 / 0.8, 0.7 | 33.4, 55.7 / 0.0, 0.0 |
| 100 panes printing: frames a second, frame build p50 | 58.8, 59.4; 2.82, 2.78 ms | 59.1, 58.1; 2.93, 3.30 ms |
| `frame_bench` (on the host, load about 16), model build p50 | 0.184, 0.203, 0.193 ms | 0.184, 0.190, 0.185 ms |

![Key to echo painted before and after GPUI refactor phases 5 to 7, in a VM: the same within the runs' spread, idle 26.0 against 28.3 ms, one pane flooding 17.5 against 20.8 ms, eight panes busy 18.0 against 18.0 ms](images/perf/gpui-refactor.svg)

What it shows: key echo did not change beyond the runs' spread. A typed key now reaches the pane
through the input handler (phase 7) no later than it did from `key_down`, and with eight panes
busy the slowest echoes came down (p99 52 to 38 ms, outside p50 99 to 81 ms). The still app uses
less CPU (about 1.5% to about 0.9% of a core in the VM) and nothing when hidden; one pane working
still costs about as much, its pulse re-laying out the window. Frames under 100 printing panes
are the same. `flood.sh` measured nothing useful in the VM: the capture's own reference window
ran from 1.4 to 31 frames a second as the host's load went to 30, and frame build p50 stayed
under 2 ms in both builds. `frame_bench` and the speed bench measure code these phases did not
touch (the server, the CLI and the render model have no diff); `frame_bench` agrees.

What still costs, and why the refactor left it: on the host a key's echo is painted about 12 ms
after the key and reaches the screen about 50 ms after it (the table at the top), most of it
waiting for GPUI's next display tick and then its presentation. The refactor's rule was to change
nothing inside GPUI (R11). Drawing the echo at once, as a probe on a branch did, took the host's
outside p50 from 49.8 to 36.3 ms, but GPUI 0.2.2 leaks a display link per immediate draw, so it
needs a change inside GPUI. The ADR took one on 2026-11-20 ([Key echo at once](#key-echo-at-once));
idle CPU (under 1% of a core shown, nothing hidden) was not a reason to.
