# Architecture: how leon uses GPUI

Part of [docs/architecture.md](../architecture.md). The app (`leon-app`, the `gui` feature) is a
GPUI client of the server; this is how it uses GPUI, as rules a contributor can check, and why.
Written 2026-11-17. A GPUI upgrade amends this file first (the doc test reads `Cargo.lock`):

Built against: GPUI 0.2.2 from crates.io.

## Context

leon's app uses GPUI's current API (`Render`, `cx.new`, `cx.notify()`, actions, focus handles,
executors), but not its shape: the window is one `Workspace` view drawn through helper functions (`view_*.rs`), with no
child views and no caching. The performance baseline (docs/performance-app.md) found a spinner frame
re-laying out the whole window (6.6% of a core then, 4.4% once the poll ran only when woken),
key echo a display frame behind another terminal, and 0.74% of a core for an idle visible window. The fixes reached for next were AppKit
tricks (`setNeedsDisplay:` on GPUI's view) and GPUI patches. This ADR is what GPUI's own source
and Zed's use of it say instead.

What GPUI 0.2.2 does, read from its source, and how Zed shapes a window with it, are in
[gpui-0.2.2.md](gpui-0.2.2.md): the facts these rules rest on (a frame, the display link, the root
rendering on every frame, cached views, notify while drawing, animation, threads, tests).

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
  frame it draws, so whatever the root does runs at display rate during any animation. Check: review, and
  a test counts what a spinner tick renders.
- **R2. Never schedule or force a frame through AppKit** (`setNeedsDisplay:`, `displayLayer:`, the
  NSView or layer behind the window handle). Use `cx.notify()` for changed state and
  `window.request_animation_frame()` for animation. Why: GPUI's `displayLayer:` path rebuilds the
  display link and 0.2.2 leaks one per call (500 keys left 501 links); frames are GPUI's job. Check: test.
- **R3. A part of the window that changes on its own clock is its own view**, shown from its
  parent with `AnyView::from(entity).cached(style)`: each pane, the sidebar, the title bar, the
  status indicator. The overlays are views too (the palette with its switcher, the overview, the
  notice; the find bar and the rename field are parts of the header and sidebar sections), but
  not cached: they fade and scale on the root's frames, and GPUI renders the root on every frame
  it draws, so a cache would be redrawn each frame of their motion anyway. Why: a
  notify dirties only that view and its ancestors, and cached siblings reuse their layout and
  paint, so a spinner frame no longer lays out the panes. A view that changes often and the heavy views beside it
  are siblings under a parent that is not itself a re-rendering cached view: a pane's header (with
  its spinner) and its grid are two cached views side by side, never the grid inside a cached
  view that also holds the spinner, whose every tick would re-render the grid. Keep hover and pressed styles, tooltips and focus changes off
  hot paths: each refreshes the window and drops every cache for that frame. A cached view's root
  element is laid out as a root, so it needs its own size (`w_full`, a height) or it
  shrinks to its content (#44). Check: review, and a render-count
  test per cached view.
- **R4. Notify the view whose drawing changed, and only it.** What a view draws lives in that
  view or in an entity it observes (`cx.observe(&model, |_, _, cx| cx.notify())`) or subscribes
  to (`EventEmitter`), keeping the returned `Subscription` in the view (dropping it ends the
  observation); a cached view that reads a model it does not observe stays stale when
  the model notifies (GPUI draws a frame but reuses the view). A window-wide hash of what
  was drawn (`Workspace::signature`) is not how a view learns it changed. Why: GPUI skips a
  cached view unless that view was notified, and a hash over everything costs every view on
  every change. Check: review, and for each input a view draws from, a test that changing it
  renders the view again.
  The model's shape (phase 5): each machine's model is a `HostModel` entity whose copy changes
  only on the main thread (`Remote::commit`), emitting what moved (sessions, screens by block,
  a keymap change); the workspace subscribes. The sidebar, title bar and pane headers are cached
  views that render through the workspace (`ws.update`, the helpers as they are) and hold a
  typed input of exactly what their helper reads, since that input mixes the model with window
  state (pane rects, the hover); on each model event the workspace notifies one only when its
  input changed (debug builds check the stored input against a fresh one), a grid on its own
  screen's event, and the root only when its own typed input (`RootInput`: sessions, tabs,
  revisions, machines, the selection) changed.
- **R5. `render`, `prepaint` and `paint` only draw.** No server calls, fetches, model changes,
  focus events or I/O in them; that work happens where the state changes (an event, an action,
  the reader's wake). The one exception: an element fitting its own content, as a pane's grid
  asks the server for its size from `prepaint` (Zed's terminal element does the same); paint
  notes where it drew (hit tests, the cursor shader's feed read that). Why: a
  cached view's `render` does not run, so work done there stops happening; a dirty one runs it
  every frame. And a `cx.notify()` made while drawing asks for no frame, so its change waits.
  Bookkeeping about the frame itself (that one was drawn, what it showed, such as the pane the
  user arrived at) may be scheduled from `render` with `cx.defer_in`, given values read in
  `render` (the model can move before it runs), never model work or I/O: it runs once `Window::draw` returns, outside the draw
  phase, on macOS and on the test platform (`Window::defer`), where `window.on_next_frame` never runs.
  Check: review.
- **R6. Animate through GPUI.** A transition with a fixed length (a slide, a fade) uses
  `with_animation` or `request_animation_frame` in the view that moves; a slow periodic change
  (the spinner, the Activity list's waited times) is a `cx.spawn` loop on
  `background_executor().timer()` that notifies its own view; indicators that move together share
  one clock entity they all observe, so N spinners cost one frame a tick, not N. Reduce Motion
  still turns movement off. Why: the cost of an animation is the subtree of the view it
  notifies; a clock in the window's poll redraws the whole window. Check: review, and a test that several working indicators draw no
  more frames a second than one.
  Motion reads `cx.background_executor().now()` (faked in tests); the root, which draws the
  drawers and overview and feeds the grids their fades, asks for frames while any moves.
  Not `with_animation`: its `Instant::now()` start ignores the test clock and restarts a
  motion turned back.
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
  threads). The window's calls to every server, This Mac's too, go through the remote's one
  ordered queue: a link's thread, or for This Mac a future the window runs on GPUI's background
  executor (`Remote::call_queue`, so tests run it on GPUI's test scheduler); a reply the window uses comes back on a wake or
  to a task awaiting it, and a call made from another's reply is one job (`act_after`), so
  nothing queued meanwhile comes between them. Why: entities are main-thread only, and a
  blocked main thread stalls every window and every frame. Check: test (no thread, sleep or
  `block_on` in GPUI code; `gpui_local_never_waits`); review for blocking calls.
- **R9. GPUI owns actions, menus and focus; leon's keymap owns the terminal's keys.** Menu items,
  ⌘Q and anything GPUI dispatches are `actions!` handled with `on_action` under a `key_context`;
  each view that takes keys has its own `FocusHandle` (`track_focus`, `window.focus`) and key
  context, takes its keys from its own `on_key_down`, and gives the focus back when it closes:
  the workspace gives the focus to the top open overlay after every change, else to itself. The
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
  R4 are for. Motion tests advance the clock and draw the frames the app asked for (R5). Check: review.
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
entry touches GPUI's window, view or layer, but `view_glass.rs`'s and `cursor_shader/overlay.rs`'s,
which say why.

- `input.rs`: `[NSApp currentEvent]` during GPUI's own key dispatch, for the key code and
  characters (GPUI rebuilds a key-code-0 event's text from the layout, so automation's keys
  arrive wrong) and the event's timestamp (the frame stats). GPUI's `KeyDownEvent` has neither.
- `notify_mac.rs`: `UNUserNotificationCenter` (post, and the click delegate, which only queues
  the click and calls the wake) and the Dock tile's badge. GPUI 0.2.2 has no notifications and
  no badge.
- `view_floats.rs`: `NSWorkspace.accessibilityDisplayShouldReduceMotion`. GPUI has no Reduce
  Motion setting.
- `view_floats.rs`: `NSWorkspace.accessibilityDisplayShouldReduceTransparency` (amended
  2026-11-17 for the translucent window). gpui-0.2.2/src has no accessibility API at all. A read
  on the main thread, again each time the window becomes active; when it is on, the window goes
  opaque through GPUI's own `Window::set_background_appearance`. It never drives a frame.
- `view_glass.rs`: the blur on macOS 27 (amended 2026-11-17). GPUI 0.2.2's blur view (class
  `BlurredView`) uses the `Selection` material, which doesn't blur there. Right after each
  `set_background_appearance(Blurred)`, a call that makes a new view after an `Opaque`, leon sets
  that view's material to `UnderWindowBackground`, as upstream GPUI's `90d8514059` does. It runs
  on the main thread and changes nothing else. With no such class or view it does nothing and
  logs once. Remove it when a crates.io GPUI has that commit: its `blurred_view_init_with_frame`
  sets `UnderWindowBackground`.
- `cursor_shader/overlay.rs`: users' cursor shaders (amended 2026-11-19; why, and how it goes, in
  [cursor-shaders.md](cursor-shaders.md)). GPUI 0.2.2 has no custom Metal
  pass, so a `CAMetalLayer` sublayer of GPUI's view's layer (via `raw-window-handle`, here only)
  draws them on its own display link, off the main thread, only while one animates (R2 holds).

To add one: show in GPUI's source that it has no way to do it, keep it to reads or system calls
on the main thread, list the file here with its reason, and never use it to drive frames (R2).

## Alternatives considered

- **Keep one view and make it cheaper** (a smaller `signature`, a dot painted in a canvas,
  fewer spinner frames). Rejected as the plan: the root still renders and lays out the whole
  window on every frame anything moves; each trick is local.
- **Patch GPUI 0.2.2** (draw at once after input, `presentsWithTransaction`, two drawables, pause
  the idle display link). Not now: the refactor removes leon's own share first, and Zed has
  changed the rest upstream (a per-display link registry; a link that pauses when nothing is dirty). Revisit with numbers from `scripts/perf/keylat.sh` and
  `idle-cpu.sh` after the refactor, as an amendment here.
- **Move to Zed's main GPUI** (a mirror of its crates). Not now: 22 crates and 6 git
  dependencies, a pinned Rust, and a test scheduler that panics when another thread wakes the
  executor, as leon's reader thread does in its tests. Revisit when a crates.io GPUI carries
  the display-link changes, or a measured gap remains after the refactor.
- **AppKit tricks for frames** (`setNeedsDisplay:` on GPUI's view). Rejected: a display link
  leaked per key, and it fights GPUI for the frame schedule (R2).

## Consequences

- Where the app is today: a root `Render` with cached views the workspace gives their input and
  notifies on change: each pane's grid (`view_grid.rs`), and the sidebar, title bar and pane
  headers, which render through the workspace (`view_sections.rs`; R1, R3). Each machine's model
  is a `HostModel` entity (`view_feed.rs`): the reader thread writes the remote's model and
  wakes the window, the wake task commits it into the window's copy on the main thread, and the
  workspace's subscription notifies a grid for its screen, a section for its input and the root
  for its own (R4). `render`'s old model work (`sync_pane`, `run_find`, `settle`) runs in
  `prepare`, on a model event and in an observer of every notify of the workspace (R5). Each
  timed thing is a task of its own (the storm summary, a host's retry, the Activity list's
  waited times, a find's re-run, a held thumbnail, the update look), and the keymap follows the
  server's `keymap_changed` event (R6, R8). Motions run on GPUI's clock, the spinner on its own
  `Pulse` (`view_pulse.rs`; R6). Each overlay that takes keys has its own
  focus handle and key context (`Palette`, `Overview`, `FindBar`, `RenameField`; the popover's own) and its own
  `on_key_down` (`view_overlays.rs`), and `sync_focus` gives the focus to the top open one after
  every change, else to the workspace, so `key_down` is the terminal's path (the keymap,
  sequences, copy mode and hints) (R3, R9). A frame drawn without the focused element (GPUI's
  `on_focus_lost`) ends a rename whose field it hid. No `InputHandler`, so no input method (R9).
  A server call queues (R8) or, for a Lua binding, a config action or a reload, runs on a thread
  of its own; what still waits on the main thread is the first
  `session.create`, before the window draws, and a watch line's write (a socket write, bounded).
- Already in line: panes paint through `canvas` (R7); std threads only in GPUI-free modules,
  reaching the UI through a wake channel (R8); tests on `TestAppContext` (R10); GPUI from
  crates.io unpatched (R11).
- The refactor costs more types (an entity per pane, sidebar, title bar, machine, overlay).
- The root renders on every frame GPUI draws (`draw_roots`), a header's or a grid's too: what a
  model event saves is the root's notify, whose observer runs `prepare` and works out every
  child's input again; the cached children are reused either way unless their input moved.
- What GPUI itself costs stays until an upgrade: the display link's idle ticks while the window
  is visible, and a key's echo waiting for the next tick. Neither is patched (R11).
