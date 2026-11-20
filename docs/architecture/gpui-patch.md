# Architecture: GPUI 0.2.2 with one display-link fix

An amendment to [How leon uses GPUI](gpui.md) (R2 and R11), adopted 2026-11-20: the change, the
measurement that needs it, and how it goes away.

## The change

leon builds GPUI 0.2.2 from [aindong/gpui](https://github.com/aindong/gpui), through
`[patch.crates-io]` in the workspace's `Cargo.toml`, pinned by full commit. That repository has
three commits:

1. `gpui-0.2.2.crate` from crates.io, unpacked unchanged (its sha256 is the checksum crates.io's
   index records, `979b45cf…`; GPUI's `LICENSE-APACHE` with it).
2. and 3. The fix, in `src/platform/mac/window.rs` and `display_link.rs`. GPUI 0.2.2 stops a
   window's display link by dropping it and starts one by building a new one: around a frame
   drawn at once (`displayLayer:`, a window becoming key), when the window is hidden or shown
   again, and when it moves to another screen. `DisplayLink`'s `Drop` never frees its
   `CVDisplayLink` (on purpose, against a crash on the link's thread), so each leaks a link. With
   the fix a window keeps one link for its life: `stop_display_link` pauses it, and
   `start_display_link` runs the same link again, moved to the window's display if that changed.
   (Commit 2 kept the link only around a frame drawn at once, and still dropped it when hidden;
   commit 3 replaces that.) Nothing else changes, and nothing off macOS.

With it, leon's app shows a key's echo at once (R2's one exception, `view_echo.rs`): when the
screen of the pane a key went to arrives, it asks AppKit to display GPUI's view, and GPUI's own
`displayLayer:` draws that frame as the run-loop pass commits and presents it with the
transaction, instead of waiting for the display link's next tick. Once per key, and only while
the window is on screen; output with no key behind it (a flood) keeps to the display link.

## Why: the measurement

- The baseline (docs/performance-app.md) found a key's echo painted about 12 ms after the key,
  most of it waiting for GPUI's next display tick, and shown about 50 ms after it from outside,
  where a window that repaints at once is shown after 19 ms.
- #21's probe, drawing the echo through `displayLayer:` on crates.io 0.2.2: key to frame shown,
  p50 49.8 to 36.3 ms (p99 67.3 to 55.0 ms); in the app, key to echo painted 11.1 to 6.8 ms;
  floods unchanged. It leaked one `CVDisplayLink` per key (501 alive after 500 keys).
- No GPUI release fixes either (checked 2026-11-20): crates.io's newest `gpui` is 0.2.2, and
  Zed's main still draws only on its frame source's tick; its `displayLayer:` still stops and
  restarts the frame source, which no longer leaks there (below).
- This patch, in a VM against dev (docs/performance-app.md, "Key echo at once"): key to frame
  shown p50 32 to 37 ms sooner (idle 102.7 to 68.7 ms), key to echo painted about 10 ms instead
  of 14 to 30 ms.

## The check

`crates/leon-app/tests/gpui_display_link.rs` opens a GPUI window on the real platform, with its
`occlusionState` answered by the test, and counts the live `CVDisplayLink`s with `heap`, which
must stay at the window's one link in both cases. On screen: shown at once 500 times through
`view_echo.rs` and moved between screens 20 times. Off screen: not shown at once, 50 frames
drawn at once anyway, and hidden and shown 50 times. With commit 2 alone it counted 21 and 72
links; on crates.io 0.2.2 the first 500 shows alone left 109. It opens a window, so it runs with `LEON_GUI_TESTS=1` (CI's macOS job) and says it
skipped otherwise. `crates/leon/tests/gpui_adr.rs` checks the `[patch]` is this fork at a full
commit and that the ADR's Built against line names that commit.

## How it goes away

Zed replaced the per-window links with one `CVDisplayLink` per display that windows join and
leave ([zed#60696](https://github.com/zed-industries/zed/pull/60696), merged 2026-07-13), and
on macOS 14 and later uses a `CAMetalDisplayLink` that is invalidated when dropped. When a
crates.io `gpui` after 0.2.2 ships either, drop the patch:

1. Move to that release and delete the `[patch.crates-io]` entry.
2. Run `LEON_GUI_TESTS=1 cargo test -p leon-app --test gpui_display_link`. It passes: the
   release reuses its links, and the patch is gone for good. It fails: keep a fork of that
   release with the same fix, as here.
3. Amend gpui.md (Built against, R11) and this file; R2's exception stays as long as GPUI draws
   only on its tick.

An upgrade while the patch stands starts the fork again: commit 1 is the new crate unpacked,
commit 2 the fix carried over, and the new commit goes in `[patch.crates-io]`.

## Costs

- Every build fetches github.com/aindong/gpui as well as crates.io, on Linux too (cargo reads
  every package in the lock); Linux never compiles GPUI.
- `scripts/notices.sh` finds GPUI's licence in cargo's git checkout, not the registry.
- The fix is carried by hand across GPUI upgrades until upstream's lands on crates.io.
