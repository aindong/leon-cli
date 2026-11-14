# Performance

`cargo bench -p leon --bench speed [-- --samples N]` (crates/leon/benches/speed.rs) starts a
private release-built server (temp `LEON_RUNTIME_DIR` and `LEON_STATE_DIR`, `--restore=false`)
and talks to it over its socket, the way a client does. It prints one line per measurement and a
table at the end. If `tmux` is on `PATH` it measures the same things through tmux's CLI;
otherwise it prints `tmux: skipped (not installed)`.

## What is measured

- **`session.list` round trip**: one NDJSON request and its reply on an open connection, after
  100 warm-up calls. p50 and p99 over 5 × N calls.
- **Input latency**: a block runs `stty raw echo; cat >/dev/null`, so the kernel echoes each key
  straight back. Each sample sends one `x` (`dev.leon.terminal.write`) and polls the screen
  (`dev.leon.terminal.format`) on the same connection until that `x` is in the grid. p50 and p99
  over N samples (N ≥ 200). The resolution is one poll, about one round trip (7 µs).
- **Throughput**: a block runs a writer of 100 MB of `y\n`, then prints an end marker. The time runs
  from `session.create` until `block.wait` returns. The server reports the exit only after its
  reader hits the PTY's EOF and its parser has caught up, so all 100 MB went through the emulator
  first. The bench then checks the marker is on the screen and asks the server how many bytes it
  parsed (`dev.leon.terminal.process` → `output_bytes`, printed as `server_parsed`): at least
  150000009, or it fails. The PTY turns each `\n` into `\r\n`, so 150 MB cross it, plus the
  9-byte marker. With `cat` the count is about 0.1% higher: when its output queue fills between
  the `\r` and the `\n`, the macOS tty sends the `\r` again. A bare reader sees the same (50146520
  `\r` against 50000000 `\n`), so the bench allows up to 1% over. The result is the median of
  3 runs, in MB of program output per second. Two writers:
  - `yes | head -c 100000000`: the writer the task names.
  - `cat` of a file with the same 100 MB, written in big blocks.

## Results

Apple M6 (12 cores), macOS 27, release build. Other builds and test runs were going on (load
average about 7), so the numbers move by about ±10% between runs. These are the medians of the
three "after" runs in the comparison below.

| | `session.list` round trip p50 / p99 | input latency p50 / p99 | `yes \| head` 100 MB | `cat` 100 MB |
|---|---|---|---|---|
| leon | 0.007 / 0.009 ms | 0.024 / 0.042 ms | 2.5 MB/s | 116.6 MB/s |
| tmux | not run: tmux isn't installed on this machine | | | |

For scale: at 0.024 ms, a keystroke's echo reaches the grid about 300 times faster than one 120 Hz
frame (8.3 ms). The macOS PTY's own limit for each writer, measured with a bare reader that does
nothing but `read()` (below), is about 2.5 MB/s and about 115 MB/s.

## The worst number, profiled

The 100 MB throughput was the worst number by far: 2.5 MB/s with `yes | head`. `sample` on the
server during that run showed the PTY reader thread blocked in `read()` in 3256 of 3497 samples
(93%). Leon wasn't busy; data was arriving slowly. A bare reader, with no leon involved, got the
same:

```sh
python3 -c 'import os,pty,time
pid,fd=pty.fork()
if pid==0: os.execvp("sh",["sh","-c","yes | head -c 20000000"])
t=time.time();n=r=0
while True:
    try: b=os.read(fd,65536)
    except OSError: break
    if not b: break
    n+=len(b);r+=1
print(n/r,"bytes/read",n/1e6/(time.time()-t),"MB/s of PTY bytes")'
```

That prints about 4 bytes per read and 3.8 MB/s of PTY bytes, which is 2.5 MB/s of program output.
macOS `head` writing to a tty is line-buffered, so it makes one `write()` per 2-byte line. That
run measures `head`, not the terminal; tmux or any other terminal would hit the same limit. So the
bench also runs `cat` of the same bytes. The bare reader gets about 172 MB/s of PTY bytes from it
(about 115 MB/s of output), and leon got about 99.

`sample` during the `cat` run showed where leon's time went: the reader thread spent 54% of its
samples in `read()` and 46% parsing (alacritty's `linefeed`/`scroll_up` most of it, the OSC 7501
scan a few percent). The tty buffers only about 1 KiB, and reading and parsing happened one after
the other on one thread. So each time the reader stopped to parse, the queue filled and `cat`
blocked.

## The change, before and after

Commit `perf: read the PTY on a thread of its own…`: a thread per terminal now only reads the PTY
into a bounded queue (64 reads of up to 16 KiB, so at most 1 MiB waits). The parser thread feeds
whatever has queued, up to 64 KiB, to the tracker and emulator under one lock and reports once per
batch. The PTY keeps draining while the emulator works.

The two builds were run alternately, three rounds each, on the same loaded machine
(`--samples 300`):

| | `cat` 100 MB (3 rounds) | `yes \| head` 100 MB | input latency p50 / p99 | round trip p50 / p99 |
|---|---|---|---|---|
| before | 96.3, 98.8, 101.3 MB/s | 2.5, 2.6, 2.5 MB/s | 0.038 / 0.072, 0.023 / 0.038, 0.023 / 0.038 ms | 0.007 / 0.038, 0.005 / 0.011, 0.007 / 0.010 ms |
| after | 114.8, 116.6, 118.3 MB/s | 2.5, 2.5, 2.6 MB/s | 0.024 / 0.046, 0.024 / 0.042, 0.024 / 0.042 ms | 0.007 / 0.010, 0.007 / 0.009, 0.007 / 0.009 ms |

`cat` went from about 99 to about 117 MB/s (+18%), which is about the bare PTY's limit. Input
latency and round trip didn't change beyond the noise: the extra thread hop costs less than one
poll. `yes | head` stays at the limit of `head` itself.

## After merging main (the app and the Lua runtime)

The merge brought M6's per-batch `generation` counter for the renderer and M5b's config thread
into the server (the bench pins `LEON_CONFIG` to a missing file, so no `init.lua` runs). Four
alternating rounds against the last pre-merge commit, in both orders, on a machine with a load
average of 4 to 12:

| | `cat` 100 MB (4 rounds) | `yes \| head` 100 MB | input latency p50 | round trip p50 |
|---|---|---|---|---|
| before the merge | 112.5, 111.1, 105.7, 108.9 MB/s | 2.5 MB/s | 0.024, 0.023, 0.038, 0.024 ms | 0.005–0.007 ms |
| after the merge | 100.7, 109.9, 109.1, 116.7 MB/s | 2.3–2.5 MB/s | 0.030, 0.036, 0.029, 0.023 ms | 0.005–0.007 ms |

Both builds' medians are about 110 MB/s for `cat`. Whichever build ran first in a round did
better (the pre-merge build ran first in rounds 1 and 2, the merged build in rounds 3 and 4), so
the spread is the machine's load, not the merge; nothing moved.

## M10: the attached app

Since M10 the app is a socket client: the server pushes each visible pane's screen (a whole one,
then the changed rows) and the app paints what it receives (docs/architecture.md, "Screen
transport"). The bench now also measures that path with the app's own client
(`leon_app::remote`) against the release server, and fails (exit 1) when the budget is missed:

- **Keystroke to screen**: one `x` every 10 ms (faster than key repeat) into the raw-mode echo
  program, timed from the write until the client has applied the pushed screen that shows it and
  woken the UI. GPUI paints on the next display frame after that; `LEON_FRAME_STATS=1` measures
  the paint itself. Budget: p50 < 4 ms, p99 < 16 ms.
- **`seq 1 200000`** in a 40 × 110 pane (about the default window), from the keystroke that starts
  it until the screen shows the end marker after it: attached (screens pushed over the socket)
  against in-process (an embedded server whose grid is read every 8 ms, as the app did before
  M10), three alternating runs each. Budget: attached at most 1.5x in-process.

The old app path's time was also measured on `00e956d`, before any M10 change, with
`frame_bench` (the same 8 ms poll, until the program exits): 181, 189, 190 ms with a load average
of 13, and 184, 177, 177 ms in a later round. So the in-process numbers below are the old path's,
not a slowed-down copy of it.

| | `session.list` round trip p50 / p99 | input latency p50 / p99 | `yes \| head` 100 MB | `cat` 100 MB | keystroke to screen p50 / p99 | `seq 1 200000` in-process / attached |
|---|---|---|---|---|---|---|
| before (`00e956d`) | 0.010 / 0.033 ms | 0.024 / 0.048 ms | 2.5 MB/s | 123.6 MB/s | (no attached path) | 177–190 ms (frame_bench) |
| after, run 1 | 0.005 / 0.023 ms | 0.024 / 0.057 ms | 2.6 MB/s | 125.6 MB/s | 0.420 / 0.784 ms | 194 / 165 ms (0.85x) |
| after, run 2 (tests running alongside) | 0.004 / 0.013 ms | 0.029 / 0.055 ms | 2.6 MB/s | 123.5 MB/s | 0.237 / 0.734 ms | 178 / 169 ms (0.95x) |
| after the review fixes | 0.005 / 0.021 ms | 0.026 / 0.054 ms | 2.6 MB/s | 121.5 MB/s | 0.330 / 0.799 ms | 188 / 166 ms (0.88x) |

The budget holds with room to spare: a keystroke's echo is on the app's screen model in under a
millisecond, and `seq` is no slower attached, a little faster even, because a pushed screen
arrives as soon as the output is parsed instead of at the next 8 ms poll. The existing rows did
not move beyond the noise: the change counter became a `tokio::sync::watch` (one more notify per
parsed batch) without a cost `cat` can see. One block's screens are at least 2 ms apart, so a
flood costs the server at most 500 frame builds a second under the terminal's lock; nothing else
needed tuning, so there is no shared memory or extra coalescing.

## M16b: hosts through a link

The app reaches a host through a `Link` (docs/architecture.md, "Remote hosts"): each connection
is an ssh process whose stdin and stdout carry the NDJSON. The bench now also measures keystroke
to screen that way, with the app's client (`Remote::connect_retrying`) on a `Link` whose
`LEON_SSH` is a fake ssh. The fake skips ssh's options and the host and runs the host's command
here, so `leon server proxy` bridges to the same release server: the path has every process and
pipe a real link has (client → link thread → ssh's stdin → proxy → server, and back), with no
network. A real link adds its round trip on top. Recorded, not budgeted.

Apple M6 (12 cores), macOS 27, release build, `--samples 200`, with other workers' builds and
tests running (load average 7 to 12, which is why `cat` is down from about 117 MB/s):

| | `session.list` round trip p50 / p99 | input latency p50 / p99 | `yes \| head` 100 MB | `cat` 100 MB | keystroke to screen p50 / p99 | through a link p50 / p99 | `seq 1 200000` in-process / attached |
|---|---|---|---|---|---|---|---|
| M16b | 0.010 / 0.020 ms | 0.040 / 0.063 ms | 1.5 MB/s | 77.2 MB/s | 0.121 / 0.279 ms | 0.137 / 0.243 ms | 298 / 286 ms (0.96x) |

Through a link, a keystroke's echo is on the app's screen model in 0.137 ms at the median, about
as fast as on the local socket: the two extra processes and four pipe hops cost microseconds. The
attached budget (keystroke p50 < 4 ms, p99 < 16 ms, `seq` at most 1.5x) still holds.

What keeps a slow link from stalling the window:

- **Screens coalesce.** The reader thread applies each pushed screen to that block's frame in the
  model, so a burst leaves only the newest; the server already sends a block's screens at least
  2 ms apart and only dirty rows. The reader wakes the UI with at most one wake queued, and the UI
  redraws once per wake when what it draws changed.
- **No call waits on the UI thread.** Over a link, keys, resizes, the focus event, pastes and
  layout changes go through one queue thread per host (in order), and a layout change's refetch
  runs there too. m16b_slow_link stops the transports (SIGSTOP), then types 20 keys, splits, opens
  a tab, steps to it and pastes: the UI thread is back in well under the test's 1 s bound (the
  synchronous path waited up to 5 s per call), and everything arrives, in order, once the
  transports run again.
- **A half-dead link is noticed in about 15 s** (the watchdog's ping), not ssh's 45 s, and the
  stream resumes on new transports; m16b_slow_link checks that within 25 s.

## Not measured yet

- tmux, because it isn't installed here. The bench's tmux path drives tmux through a process per
  command (`send-keys`, `capture-pane`, `list-sessions`), as a shell script would, so its round
  trip and latency include process start-up. Its throughput is timed until the writer finishes,
  not until tmux has drawn the output.
- Latency through a real shell's line editor, and GPUI's paint in the keystroke number (the
  display's next frame comes after the measured point).
- A real network link: the link number above has no network in it. Over Wi-Fi or a VPN each
  keystroke adds one network round trip (the write out, the pushed screen back).
