# Cursor shaders: an overlay of leon's own above GPUI

Part of [docs/architecture.md](../architecture.md), and an amendment to
[How leon uses GPUI](gpui.md) (its R2, R4, R5 and "Allowed outside GPUI"). Written 2026-11-19, for
GPUI 0.2.2. How to use the feature is in [docs/cursor-shaders.md](../cursor-shaders.md).

## Context

Users asked for Shadertoy-style cursor shaders: a GLSL file that defines `mainImage` over a set
of uniforms (the cursor's rect, its previous rect, when it changed) and draws a trail, a smear or
a glow as the cursor moves. Such a shader is a pass over the whole window's picture, and GPUI
0.2.2 has no way for an element to add a custom Metal pass or a post-process: its renderer
(`platform/mac/metal_renderer.rs`) draws only its own primitives (quads, shadows, paths, sprites,
surfaces), compiled from its own `shaders.metal`. Two other ways were declined by the user: a
built-in cursor animation (not the user's shaders) and a GPUI patch (R11).

A spike (2026-11-18, branch `aindong/shader-spike`) measured the overlay below with 11 published
cursor shaders: all 11 translated with naga and compiled with Metal at runtime; 9 matched the
reference picture within 3/255 over the overlay, one was close (a glow past 1.0), and one reads
the picture under it (it can't work here, below).

## Decision

**A transparent `CAMetalLayer` of leon's own, a sublayer of GPUI's view's layer**, drawn by a
`CVDisplayLink` of its own that runs only while a cursor change animates. Code:
`crates/leon-app/src/cursor_shader/` (`wrap.rs` the contract and naga, `program.rs` Metal,
`cursor.rs` the uniforms, `overlay.rs` the layer and link) and the workspace's side in
`view_cursor_shader.rs`.

- **The layer.** `overlay.rs` takes GPUI's `NSView` from `raw-window-handle` and adds one
  sublayer to its layer (BGRA8, not opaque, `autoresizingMask` width and height, so it follows
  GPUI's layer through resizes). It never changes GPUI's layer or view otherwise and never calls
  `setNeedsDisplay:` or `displayLayer:`: frames for GPUI stay GPUI's (R2 holds). A sublayer is
  not a view, and AppKit hit-tests and sends events to views only, so it never takes a click or
  a key. Its frame, Retina scale and drawable size are set on the main thread from the
  workspace's bounds observer (resize, full screen, another display) and at each animation's
  start. One per window; GPUI's test windows have no AppKit view, so there it keeps the cursor
  state only, which tests read.
- **Its own display link**, started by a cursor change and stopped by itself one clear frame
  after `cursor_shader_duration` (default 1 s), or at once (after a clear frame) when the window
  stops being active, Reduce Motion is on, or the overview or the palette opens. Idle, it is
  stopped: `scripts/idle-frames.sh --cursor-shader FILE` counts its frames and fails on any after
  the animation. An occluded window that stays key gets no new cursor (GPUI stops drawing it),
  so the link stops on its own within the duration; GPUI 0.2.2 has no occlusion observer.
- **Off the main thread.** The overlay draws on the link's thread; the main thread never waits
  on `nextDrawable`. The spike drew the first frame from GPUI's paint instead, to land the trail
  in the same frame as GPUI's cursor: key to echo went from 8.5 ms to 31.9 ms at p50
  (`nextDrawable` blocked up to 31 ms). So the trail is drawn on the overlay's own link, up to a
  display frame apart from GPUI's cursor (16 ms at 60 Hz), visible only as the first frame of a
  long jump. A debug assertion in
  `overlay.rs` holds it.
- **The cursor comes from the model's events, placed where the grid painted** (amended
  2026-11-19, GPUI phase 5). The workspace hands the overlay the focused pane's cursor (a value
  written under a lock; the link started if it was stopped) after a pushed screen brought that
  pane's grid up to date (its model event) and after each frame drawn (`drew_frame`, R5's frame
  bookkeeping: a slide, a tab switch, a float moved). The cell, style and colour come from the
  grid's input, through the same function paint draws the cursor with (`cursor_drawn`); the
  rect from where the grid last painted, the record paint keeps for hit tests. So nothing is fed
  from paint, and the overlay can start a frame before GPUI draws the moved cursor, where it
  started a frame after. A grid not painted yet feeds nothing until it is. Only the focused pane
  of the active window feeds (`cursor_feeder`), and a focused tiled pane doesn't while a float
  is shown (the overlay is above everything GPUI draws). A cursor in another pane is a jump,
  not a move, and so is the same cell drawn somewhere else (the pane sliding).
- **The matte.** The overlay never sees GPUI's picture, so `mainImage` runs twice per pixel, over
  flat opaque black and over flat opaque white; for a shader that blends over the picture
  (`mix(base, c, a)`), black is `c·a` and white is `c·a + 1 − a`, which give the premultiplied
  colour and its coverage, whatever the shader does with alpha. A shader that reads the picture
  (CRT, bloom, `1 − texture(iChannel0, uv)`) can't work, and `leon config check` can't tell one.
- **naga** (already GPUI's build dependency) translates GLSL to MSL, and Metal compiles it at
  runtime (no Metal Toolchain needed). Two naga 25 bugs are worked around in leon's wrapper, each
  with a test that fails once naga fixes it: a float array in a std140 block gets a 4-byte stride
  (`iChannelTime` is kept out of the block), and a global's non-constant initializer is never
  stored (leon hoists it into `main`, keeping the file's line numbers).

## Measurement

From the spike (2026-11-18): GPU 0.07–0.6 ms a frame at 2880×1800 for the shaders tested (one
noise-heavy shader 3.9 ms); key to echo p50 +2.8 ms with the overlay animating, at load 5–6.
From this build (2026-11-19, `scripts/perf/keylat.sh --loads idle --keys 150` with
`PERF_FRONT=1`, at load 3–3.5: other builds kept it from going under 2): key to echo painted
p50 10.3 ms and p99 18.9 ms with no shader, 10.0 and 19.9 ms with cursor_smear animating through
the whole run (1,194 overlay frames), so no measurable cost to typing; and
`scripts/idle-frames.sh --cursor-shader` draws one animation, then nothing while idle.

## How it goes away

When GPUI lets an element add a Metal pass (a custom shader primitive, or a post-process hook
over the frame it draws), the shader moves into that element: drawn in GPUI's own command
buffer, in the same frame as the cursor, over the real picture (so the matte goes and shaders
that read the picture work), with no layer or display link of leon's. `overlay.rs` and its
entry under gpui.md's "Allowed outside GPUI" are then deleted; `wrap.rs`, `cursor.rs` and the
settings stay.

## Linux (future work, no code)

GPUI 0.2.2 draws with blade (Vulkan) on Wayland and X11. On **Wayland** the same overlay is a
`wl_subsurface` above GPUI's `wl_surface`, desynchronized, with an empty input region (it takes
no pointer or touch), its own Vulkan swapchain with pre-multiplied composite alpha, driven by
`wl_surface.frame` callbacks while animating, and following GPUI's size and fractional scale;
naga's SPIR-V writer replaces the MSL one, and the wrapper and hoist are shared. On **X11** a
child window needs an ARGB visual and blends over its parent only under a compositing manager;
without one it shows black. Wayland first, X11 without cursor shaders. Until then `leon config
check` off macOS says cursor shaders are macOS only for now.

## Alternatives considered

- **Draw the first frame in GPUI's paint** to remove the one-frame lag: stalls typing (above).
- **A built-in cursor animation** in GPUI's own drawing: declined (not the user's shaders).
- **Patch GPUI** to add a custom pass: declined (R11), and the overlay needs no patch.
- **glslang and SPIRV-Cross** instead of naga: tens of thousands of lines of C++ in every build;
  naga is pure Rust and already built for GPUI. Kept as the fallback if naga's GLSL reader turns
  out too strict for the files users bring.
