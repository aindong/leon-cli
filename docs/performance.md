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

## Not measured yet

- tmux, because it isn't installed here. The bench's tmux path drives tmux through a process per
  command (`send-keys`, `capture-pane`, `list-sessions`), as a shell script would, so its round
  trip and latency include process start-up. Its throughput is timed until the writer finishes,
  not until tmux has drawn the output.
- Latency through a real shell's line editor, and rendering in the app (M6).
