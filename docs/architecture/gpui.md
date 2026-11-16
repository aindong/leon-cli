# Architecture: how leon uses GPUI

Part of [docs/architecture.md](../architecture.md). The app (`leon-app`, the `gui` feature) is a
GPUI client of the server; this is how it uses GPUI, as rules a contributor can check, and why.
Written 2026-11-17. A GPUI upgrade amends this file first (the doc test reads `Cargo.lock`):

Built against: GPUI 0.2.2 from crates.io.

## Context

leon's app uses GPUI's current API (`Application::new().run`, `Render` with `&mut Window` and
`&mut Context<Self>`, `cx.new`, `cx.notify()`, actions, focus handles, executors), but not its
shape: the window is one `Workspace` view drawn through helper functions (`view_*.rs`), with no
child views and no caching. The performance baseline (docs/performance.md) found a spinner frame
re-laying out the whole window (6.6% of a core then, 4.4% since the poll runs only when woken), key echo about one display frame behind another
terminal, and 0.74% of a core for an idle visible window. The fixes reached for next were AppKit
tricks (`setNeedsDisplay:` on GPUI's view, which leaked a display link per key) and GPUI patches.
This ADR is what GPUI's own source and Zed's use of it say instead.

What GPUI 0.2.2 does, read from its source (`~/.cargo/registry/src/*/gpui-0.2.2/src`):

- **A frame.** `cx.notify()` only marks the view dirty (`App::notify`, `WindowInvalidator`). On
  macOS a frame is presented only from `on_request_frame` (`window.rs`), called by the window's
  `CVDisplayLink` tick (`platform/mac/window.rs` `step`), by `displayLayer:`, on activation and on
  resize. If the window is dirty it runs `Window::draw` and presents; otherwise, for 1 s after
  input to the active window, it presents the last frame again (no new draw). `Window::draw`
  also runs, without presenting, before a key is dispatched to a dirty window
  (`dispatch_key_event`), when a window opens, and on each flush in tests.
- **The display link ticks at the display's refresh rate whenever the window is visible**, dirty
  or not (`start_display_link`; occlusion stops it and visibility starts it). That is the idle 0.74%. `displayLayer:` and
  activation drop the link and build a new one; in 0.2.2 each rebuild leaks one.
- **The root view renders on every frame** (`draw_roots`). `cx.notify()` on a view marks it and
  its ancestors dirty (`mark_view_dirty`). A child shown through `AnyView::from(entity).cached(style)`
  reuses last frame's layout and paint unless it is dirty, the window is refreshing, or its
  bounds, content mask or text style changed (`view.rs`, `AnyView` `prepaint`; so a cached view
  that moves draws again), and its subtree is not laid out again (its style is the whole layout
  node). A cached view that does render again renders its
  subtree with caching off (`window.refreshing`), so cached views nested in it render too:
  caching pays only for the cached children of a parent that did not render again.
  `Window::refresh` ignores every cache, and GPUI calls it itself when a hover or pressed
  style, a drag, a tooltip or the focus changes (`elements/div.rs`, `Window::focus`).
- **A notify while drawing schedules nothing.** During `render`, `prepaint` or `paint` a
  `cx.notify()` marks the view but does not ask for a frame (`WindowInvalidator::invalidate_view`
  returns false outside `DrawPhase::None`), so the change waits for some other wake.
- **Notifying a model is not enough for a cached view.** An entity read while drawing makes the
  window draw again when it notifies, but only views in the dirty set skip their cache; a view
  that shows a model observes it (`cx.observe`) or subscribes to its events (`cx.subscribe`) and
  notifies itself.
- **Animation.** `with_animation` (`elements/animation.rs`) and `window.request_animation_frame()`
  notify the view they run in on the next frame (`on_next_frame`), so an animation costs that
  view's subtree, every tick, until it ends.
- **Lists and grids.** `uniform_list` and `list` render only the visible rows; `canvas` and custom
  `Element`s paint without per-cell layout.
- **Threads.** Entities live on the main thread. Work leaves it through `cx.spawn` and
  `cx.background_executor()` (`spawn`, `timer`); other threads come back through a channel a
  `cx.spawn` task awaits.
- **Tests.** `#[gpui::test]` gives a `TestAppContext` on a test platform; `add_window_view` gives a
  `VisualTestContext` with `simulate_*` input and `run_until_parked`.

How Zed shapes a window with it (Zed at
[69e2130](https://github.com/zed-industries/zed/tree/69e2130295c2649963eb639fc70b4f2ee8ea1624),
the commit 0.2.2 was published from): the workspace is a view whose panes and dock panels are
entities, each shown as `AnyView::from(pane).cached(…)` (`workspace/src/pane_group.rs`,
`dock.rs`); a terminal is a model entity fed by alacritty's thread through a channel awaited in
`cx.spawn` and batched per 4 ms, which emits `Wakeup`, and its view subscribes and notifies
(`terminal/src/terminal.rs`, `terminal_view.rs`); the grid is one custom `Element` that batches
text runs per row and takes text input through an `InputHandler` (`terminal_element.rs`); the
cursor blink is a `cx.spawn` timer loop that notifies its own view (`editor/src/blink_manager.rs`).

## Decision

leon's app uses GPUI as GPUI is built to be used: a thin root view; a view (entity) per part of
the window that changes on its own clock, shown cached; state in entities that notify the views
that draw it; animation through GPUI; painting through elements; GPUI's executors for async; and
GPUI's actions, key contexts and focus for what GPUI owns. Code beside GPUI (AppKit through
objc2, std threads) is allowed only where this file lists it. GPUI is used as published,
crates.io GPUI 0.2.2, with no fork or patch, unless an amendment here names the change, the
measurement that needs it, and how the patch goes away. The app moves to this shape in phased
PRs (the plan in the PR that added this file); until a rule's phase lands, code that is changed
follows the rule and code that isn't stays as it is.

## Rules

Each rule says how it is checked: **test** (`crates/leon/tests/gpui_adr.rs`, or the app test the
rule names) or **review** (the reviewer reads the diff against the rule and says so).

- **R1. The root view is thin.** `Workspace` holds the window's state and arranges child views;
  its `render` does no per-pane work and no model work. Why: GPUI renders the root view on every
  frame it draws, so whatever the root does runs at display rate during any animation. Check: review; once
  the indicators are views, a test counts what a spinner tick renders.
- **R2. Never schedule or force a frame through AppKit** (`setNeedsDisplay:`, `displayLayer:`, the
  NSView or layer behind the window handle). Use `cx.notify()` for changed state and
  `window.request_animation_frame()` for animation. Why: GPUI's `displayLayer:` path rebuilds the
  display link and 0.2.2 leaks one per call (500 keys left 501 links); frames are GPUI's job. Check: test.
- **R3. A part of the window that changes on its own clock is its own view**, shown from its
  parent with `AnyView::from(entity).cached(style)`: each pane, the sidebar, the title bar, the
  status indicator, and each overlay (palette, switcher, find bar, notice, overview). Why: a
  notify dirties only that view and its ancestors, and cached siblings reuse their layout and
  paint, so a spinner frame no longer lays out the panes. A view that changes often and the heavy views beside it
  are siblings under a parent that is not itself a re-rendering cached view: a pane's header (with
  its spinner) and its grid are two cached views side by side, never the grid inside a cached
  view that also holds the spinner, whose every tick would re-render the grid. Keep hover and pressed styles, tooltips and focus changes off
  hot paths: each refreshes the window and drops every cache for that frame. Check: review, and
  a render-count test per cached view.
- **R4. Notify the view whose drawing changed, and only it.** What a view draws lives in that
  view or in an entity it observes (`cx.observe(&model, |_, _, cx| cx.notify())`) or subscribes
  to (`EventEmitter`), keeping the returned `Subscription` in the view (dropping it ends the
  observation); a cached view that reads a model it does not observe stays stale when
  the model notifies (GPUI draws a frame but reuses the view). A window-wide hash of what
  was drawn (`Workspace::signature`) is not how a view learns it changed. Why: GPUI skips a
  cached view unless that view was notified, and a hash over everything costs every view on
  every change. Check: review, and for each input a view draws from, a test that changing it
  renders the view again.
- **R5. `render`, `prepaint` and `paint` only draw.** No server calls, fetches, model changes,
  focus events or I/O in them; that work happens where the state changes (an event, an action,
  the reader's wake). The one exception is an element fitting its own content, as a pane's grid
  asks the server for its size from `prepaint` (Zed's terminal element does the same). Why: a
  cached view's `render` does not run, so work done there stops happening; a dirty one runs it
  every frame. And a `cx.notify()` made while drawing asks for no frame, so its change waits.
  Bookkeeping about the frame itself (that one was drawn, what it showed, such as the pane the
  user arrived at) may be scheduled from `render` with `cx.defer_in`, given values read in
  `render` (the model can move before it runs), never model work or I/O: it runs once `Window::draw` returns, outside the draw
  phase, on macOS and on the test platform (`Window::defer`, `window.rs:1541`; `flush_effects`,
  `app.rs:1201`), where `window.on_next_frame` never runs (`platform/test/window.rs:235`).
  Check: review.
- **R6. Animate through GPUI.** A transition with a fixed length (a slide, a fade) uses
  `with_animation` or `request_animation_frame` in the view that moves; a slow periodic change
  (the spinner, the Activity list's waited times) is a `cx.spawn` loop on
  `background_executor().timer()` that notifies its own view, as Zed's cursor blink does; indicators that move together share
  one clock entity they all observe, so N spinners cost one frame a tick, not N. Reduce Motion
  still turns movement off. Why: the cost of an animation is the subtree of the view it
  notifies; a clock in the window's poll redraws the whole window. Check: review, and a test that several working indicators draw no
  more frames a second than one.
- **R7. Big or dense content is painted, not laid out.** A pane's grid stays one `canvas` or a
  custom `Element` that shapes text runs per row and paints quads; a list that can grow long
  uses `uniform_list` or `list`. Why: GPUI lays out every element of a view that renders (taffy),
  so a div per cell or per row of a long list multiplies each frame's cost. Check: review.
- **R8. View code is async through GPUI.** Off-thread work from a view goes through `cx.spawn`
  with `cx.background_executor().spawn` or `timer`; no `std::thread` and no `thread::sleep` in
  view files, and no call that can wait on the network on the main thread. The GPUI-free modules
  (`remote*`, `hosts`, `updates`, shared with `leon attach`, which has no GPUI) keep their
  threads and reach the UI only through a channel a `cx.spawn` task awaits (the wake), as Zed's
  terminal does (the link's retry and watchdog sleeps in `remote_link.rs` are on its own
  threads). Why: entities are main-thread only, and a blocked main thread stalls every
  window and every frame. Check: test (no thread, sleep or `block_on` in GPUI code); review for
  blocking calls.
- **R9. GPUI owns actions, menus and focus; leon's keymap owns the terminal's keys.** Menu items,
  ⌘Q and anything GPUI dispatches are `actions!` handled with `on_action` under a `key_context`;
  each view that takes keys has its own `FocusHandle` (`track_focus`, `window.focus`). The
  terminal's keys go from `on_key_down` to leon's keymap (the server's Lua keymap, shared with
  `leon attach`). Text that composes (an input method, the emoji picker) goes through an
  `InputHandler` (`window.handle_input`), as Zed's terminal does. Why: GPUI dispatches by the
  focused element's context path, and delivers marked text only to an input handler. Check: review.
- **R10. Tests drive GPUI's test contexts.** `#[gpui::test]`, `add_window_view`, `simulate_*` and
  `run_until_parked`; a test of a view checks what it drew (`debug_selector`, the drawn list) or
  how often it rendered, never a fixed sleep. leon's drawn list (`view::drawn`) is filled by
  `debug_selector` while elements are built, so a reused cached view adds nothing to it: a
  cached view keeps its own record, and a test reads that. Why: GPUI's test platform is deterministic and
  has no display link, so a test can say "this view did not render again", the property R3 and
  R4 are for. Check: review.
- **R11. GPUI is used as published.** crates.io GPUI 0.2.2: no fork, patch, vendored copy or
  `[patch]` entry, unless an amendment here names it. Why: a patch is paid for again at every
  upgrade, and the GPUI-side costs the baseline found are fixed upstream (below). Check: test (the Built against line matches `Cargo.lock`, and no
  `[patch]` names gpui).
- **R12. Read GPUI before using it.** Before writing or changing GPUI code, read the source or
  an example (`gpui-0.2.2/src`, `gpui-0.2.2/examples`) for each API used, and Zed at 69e2130 for
  the pattern; no names from memory, blog posts or another version (`View`, `Model`,
  `ViewContext` are gone). Why: GPUI is pre-1.0, its docs are short, and what it does (caching,
  frames) is only in the source. Check: review (the PR says which source it read).

## Allowed outside GPUI

Raw AppKit through objc2 is allowed in these files and for these reasons only; the doc test
(`crates/leon/tests/gpui_adr.rs`) fails when another file calls `msg_send!` or `objc2::`. No
entry touches GPUI's window, view or layer.

- `input.rs`: `[NSApp currentEvent]` during GPUI's own key dispatch, for the key code and
  characters (GPUI rebuilds a key-code-0 event's text from the layout, so automation's keys
  arrive wrong) and the event's timestamp (the frame stats). GPUI's `KeyDownEvent` has neither.
- `notify_mac.rs`: `UNUserNotificationCenter` (post, and the click delegate, which only queues
  the click and calls the wake) and the Dock tile's badge. GPUI 0.2.2 has no notifications and
  no badge.
- `view_floats.rs`: `NSWorkspace.accessibilityDisplayShouldReduceMotion`. GPUI has no Reduce
  Motion setting.

To add one: show in GPUI's source that it has no way to do it, keep it to reads or system calls
on the main thread, list the file here with its reason, and never use it to drive frames (R2).

## Alternatives considered

- **Keep one view and make it cheaper** (a smaller `signature`, a dot painted in a canvas,
  fewer spinner frames). Rejected as the plan: the root still renders and lays out the whole
  window on every frame anything moves; each trick is local and the next feature undoes it.
- **Patch GPUI 0.2.2** (draw at once after input, `presentsWithTransaction`, two drawables, pause
  the idle display link). Not now: the refactor removes leon's own share first, and the rest is
  a GPUI matter Zed has changed upstream (a per-display link registry; a display link that
  pauses when nothing is dirty). Revisit with numbers from `scripts/perf/keylat.sh` and
  `idle-cpu.sh` after the refactor, as an amendment here.
- **Move to Zed's main GPUI** (a mirror of its crates). Not now: 22 crates and 6 git
  dependencies on every platform's build, the platform split into `gpui_platform` and
  `gpui_macos`, a pinned Rust, and a test scheduler that panics when another thread wakes the
  executor, which leon's reader thread does in its tests. Revisit when a crates.io GPUI carries
  the display-link changes, or when the refactor is done and a measured gap remains.
- **AppKit tricks for frames** (`setNeedsDisplay:` on GPUI's view). Rejected: it leaked a
  display link per key, and it fights GPUI for the frame schedule (R2).

## Consequences

- Where the app is today (each moved by the refactor's phases): one `Render` for the window but
  for each pane's grid, a cached `PaneGrid` view the workspace gives its input and notifies on
  change (`view_grid.rs`; headers, sidebar and title bar are phase 3; R1, R3); the model work
  `render` did (`sync_pane`, `run_find`, `settle`) runs in `prepare`, an observer of every
  notify of the workspace, and for a pushed screen in the poll (phase 5 takes it over; R5); a
  poll that hashes what the root draws (no screens) and notifies the whole window
  (`view_poll.rs`; R4); slides, fades and the spinner clocked by
  that poll (R6); keys for the palette, find and modes handled by hand inside one `key_down`
  under one focus handle (R9); no `InputHandler`, so no input method (R9); every call to This
  Mac's server waits on the main thread, since its `Remote` has no queue (`Remote::enqueue`):
  actions, each key's write, `fit` in prepaint and `watch` in `prepare` (R8).
- Already in line: panes paint through `canvas` (R7); std threads only in GPUI-free modules,
  reaching the UI through a wake channel (R8); tests on `TestAppContext` (R10); GPUI from
  crates.io unpatched (R11). The overview's thumbnails and the panes' grids are cached views.
- The refactor costs more types (an entity per pane, sidebar, title bar, overlay) and state
  that moves out of `Workspace` behind `Entity` handles; tests that read `Workspace` fields read
  the child entities instead.
- Until the model is entities (phase 5), `render` reads the live model the reader thread writes
  off the UI thread. A frame drawn between that write and the poll's wake (a hover refresh, a
  resize, the display link first) shows a model the workspace has not reconciled yet, for one
  frame: a destroyed selected session as the empty window, a row just scrolled into history
  blank in a pane scrolled back. Phase 5's entities, changed only on the main thread, close it.
- What GPUI itself costs stays until an upgrade: the display link's idle ticks while the window
  is visible, and a key's echo waiting for the next tick. Neither is patched (R11).
