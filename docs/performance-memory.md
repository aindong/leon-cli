# Performance: memory

What the server keeps after a pane closes, and what the app leaks. The machine, the scripts and
how they run are in docs/performance.md.

## Giving a closed pane's memory back: tried, dropped

A 2-hour soak showed the server's footprint swinging 667 to 896 MiB while its live heap stayed at
748 MB: freed history rows the allocator keeps. The idea was to ask the allocator to hand them back
when a pane's history is freed, with `malloc_zone_pressure_relief(NULL, 0)`, on a thread of its own
100 ms after the first close. It was built and measured, and is not in leon, because the call does
nothing for this memory on macOS 27.2:

- A C program that mallocs 40,000 blocks of 1,920 bytes (an 80-column history row), frees them,
  then calls it: 80 MiB allocated, 32 MiB of footprint left after the frees, and the call returns
  0 bytes and leaves 32 MiB, for every zone or the default zone alone, with a goal of 0 or 1 TiB.
  Blocks of 512 and 4,800 bytes behave the same.
- A server with one session of 4 panes of 10,000 lines, killed: the footprint falls from 98 to
  38 MiB with or without the give-back (live heap 0.6 MiB after; `vmmap` shows 36 MiB of free
  blocks the default zone keeps dirty, 99% fragmented, on both).
- `soak.sh 30`, dev at 9f1204f and dev with the give-back, run side by side (the same start, load
  3.7 to 10.2, private dirs each; the 12 long-lived agent panes parsed 183 and 188 MiB): the
  server's footprint over minutes 10 to 30, without the minute after a `leaks` pause, averaged
  464.1 MiB on dev and 468.3 MiB with the give-back (peak 615.3 and 616.6). Live heap 411.5 MiB
  on both at minutes 15 and 30; footprint minus live heap 41.1 and 39.4 MiB at minute 15, −11.3
  and −1.5 MiB at minute 30. The plan's bar was 50 MiB lower in mean.

The allocator already returns most of a freed history on its own as the blocks are freed (80 to
32 MiB above, even with every block free); what it keeps, the relief call does not return. Making
history cost less is a change to how it is stored (a leon-side store for old rows), not to when
memory is handed back.

## History in leon's store

History now moves out of alacritty into a store of each row's text and style runs when output
pauses (docs/architecture/scrollback.md). Measured on 2026-11-19, dev at a3edd74 against the
branch, one after the other on the same machine; it was never quiet (other builds ran), so the
load is given with each:

- `soak.sh 8`, the server's footprint by minute. dev: 359, 402, 444, 460, 523, 562 MiB at minutes 0
  to 5 (load 3.1, 3.0, 4.5, 6.1, 14.7, 18.7). Branch: 108, 58, 52, 46, 45, 40 MiB (load 3.1 to
  3.9), then 36 MiB at minutes 6 to 8. Over minutes 1 to 5: 478 MiB on dev, 48 MiB on the branch.
  An idle server with one shell is 4.9 MiB on both, so the 14 agent panes (12, and a session of 2
  made and killed every minute) cost about 33 MiB each on dev and 3 MiB on the branch: 90% less.
  At minute 0 the agents are still printing their first 10,000 lines, which alacritty holds at
  full width until they pause.
- In process, 10,000 lines of 70 characters at 80 columns (`b1_store_memory.rs`, a counting
  allocator): 19.7 MB in alacritty alone, 2.2 MB with the store (89% less).
- `cargo bench -p leon --bench speed`, three rounds each, alternating: `cat` of 100 MB at 119.0,
  115.8 and 92.8 MB/s on dev (load 2.8, 3.1, 5.1) and 127.0, 58.2 and 123.4 MB/s on the branch (load
  3.0, 3.1 rising to 6.9 during the run, 9.2); input latency p50 0.023 to 0.035 ms on dev, 0.028 to
  0.030 ms on the branch; the attached keystroke to screen p50 0.36 to 0.51 ms on dev, 0.22 to 0.51
  ms on the branch. Rows aren't copied as they scroll, only when output pauses (or once a second),
  so the parse path is as before; a first version that copied each row as it scrolled ran `cat` at
  11 MB/s.

## Two leaks in GPUI

The app's `leaks` (`MallocStackLogging=1`, release build, agents reporting status) are all
allocated under GPUI 0.2.2, none in leon's code: 44 objects (2.8 KB) after 6 minutes, a few more
an hour (56, 2.5 KB, after the 2-hour soak). leon carries no patched GPUI for that; these are the
fixes, ready to send upstream.

- `src/platform/mac/open_type.rs`, `apply_features_and_fallbacks`: the arrays from
  `generate_feature_array` and `generate_fallback_array` come from `CFArrayCreateMutable` (the
  caller owns them) and go into a dictionary that retains them, but are never released: arrays
  leak once per font resolved. Fix, after `CFDictionaryCreate(...)`:
  `for value in &values { CFRelease(*value as _); }`
- `src/platform/mac/window.rs`, `display_id_for_screen`: the `NSScreenNumber` key is made with
  `NSString::alloc(nil).init_str(...)` (owned) and never released; `start_display_link` calls it
  at every key-window and occlusion change. Fix:
  `let screen_number_key: id = NSString::alloc(nil).init_str("NSScreenNumber").autorelease();`
  `MacWindow::display` makes the same key the same way; the same `.autorelease()` fixes it.
