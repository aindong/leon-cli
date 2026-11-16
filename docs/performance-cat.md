# Performance: a big `cat`

`scripts/perf/flood.sh`'s `cat` of 100 MB of text and colour ran at 81 MB/s in leon and at 131 in
another terminal. In the server's profile leon's own passes over the output cost more than
alacritty's parse, and the parse thread waited 23% of its time for the terminal's lock while the
screen push built frames under it (docs/performance.md, "The app end to end").

## The changes

- One scanner (`terminal::Escapes`) follows the X10 mouse mode and the history clears together,
  in place of a byte loop and two `windows()` searches per 4 KiB slice. It and the OSC 7501
  scanner jump from ESC to ESC with `memchr` and step over an `ESC x` or `ESC [ x` that starts
  nothing they follow (most of a flood's escapes are SGR). The OSC scanner hands out runs of 1 KiB
  or more without copying them; shorter ones are copied as before, so escapes close together
  can't make a call into the emulator per few bytes. A history clear split across two slices or
  two reads is now seen by the scanner too (the line count's anchor search had covered it).
- A screen push copies the screen under the lock and builds the frame after it. The copy takes at
  most the 16 combining marks a frame draws per cell and shares nothing with the emulator, so a
  cell with many marks costs the parse thread nothing extra. Building rows had also been
  O(columns²): `push_cell` counted each span's chars once per cell.
- Tests: each escape split at every byte inside 5,000 bytes of text; 2,000 random streams split
  at random against the old X10 scanner and the old clear search, and against the OSC scanner fed
  a byte at a time (its per-byte path); the frame's spans for wide chars and marks, pinned from
  the old code; mutants of both skips are caught.

## Before and after

Release builds of the base (`9b56116`) and the branch (`fac56ca`), alternating, one pane, the app
showing it, Apple M6, the load beside each run:

| run | `flood.sh` 100 MB | screen current after the last byte | frames shown |
|---|---|---|---|
| base | 79.5 MB/s (load 3.48) | 38.7 ms | 56.3 fps, none missed |
| branch | 115.2 MB/s (load 2.63) | 46.5 ms | 55.8 fps, none missed |
| base | 79.7 MB/s (load 3.06) | 55.2 ms | 56.5 fps, none missed |
| branch | 114.2 MB/s (load 2.95) | 46.3 ms | 55.3 fps, none missed |

The speed bench's `cat` (`y` lines, no escapes): base 111.0 MB/s (load 2.95), branch 118.3
(load 3.44). The later fixes (short runs copied, marks not shared) don't touch a flood with no OSC
or marks beyond the cell copy.

## What is left

- `cat` is 44% faster but short of the other terminal's 131 MB/s. The parse thread now waits for
  the lock 2% of its time (a 5 s `sample` at load 3.3), and leon's own scans are about a tenth of
  its samples. The rest is alacritty's: printing (a fifth), resetting each row that scrolls in (a
  sixth, more in wider panes), SGR attributes and vte's UTF-8 checks.
- The screen is current about 46 ms after the last byte, not under 41: no better than the base,
  whose runs spread from 39 to 55 ms. Where those milliseconds go was not profiled here.
- Under load the gap is wider: at load 32, base 16 MB/s against the branch's 79, since every lock
  handover then waits for a CPU.
- Not flagged as history clears, before or after: `CSI 3;5 J`, `CSI 03 J`, or a `CSI 3 J` with a
  control byte inside, all of which alacritty treats as one.
