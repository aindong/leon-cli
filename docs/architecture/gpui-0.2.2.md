# GPUI 0.2.2 as leon uses it: what it does, and how Zed uses it

Part of [How leon uses GPUI](gpui.md) (its Context): the facts its rules rest on, read from GPUI's
source (`~/.cargo/registry/src/*/gpui-0.2.2/src`) and from Zed at the commit 0.2.2 was published
from. A GPUI upgrade checks each again.

## What GPUI 0.2.2 does

- **A frame.** `cx.notify()` only marks the view dirty (`App::notify`, `WindowInvalidator`). On
  macOS a frame is presented only from `on_request_frame` (`window.rs`), called by the window's
  `CVDisplayLink` tick (`platform/mac/window.rs` `step`), by `displayLayer:`, on activation and on
  resize. If the window is dirty it runs `Window::draw` and presents; otherwise, for 1 s after
  input to the active window, it presents the last frame again (no new draw). `Window::draw`
  also runs, without presenting, before a key is dispatched to a dirty window
  (`dispatch_key_event`), when a window opens, and on each flush in tests.
- **The display link ticks at the display's refresh rate whenever the window is visible**, dirty
  or not (`start_display_link`; occlusion stops it): the idle 0.74%. `displayLayer:` and
  activation build a new link; in 0.2.2 each rebuild leaks one.
- **The root view renders on every frame** (`draw_roots`). `cx.notify()` on a view marks it and
  its ancestors dirty. A child shown through `AnyView::from(entity).cached(style)`
  reuses last frame's layout and paint unless it is dirty, the window is refreshing, or its
  bounds, content mask or text style changed (`view.rs`, `AnyView` `prepaint`; so a cached view
  that moves draws again), and its subtree is not laid out again (its style is the whole layout
  node). A cached view that does render again renders its
  subtree with caching off (`window.refreshing`), so cached views nested in it render too:
  caching pays only for the cached children of a parent that did not render again.
  `Window::refresh` ignores every cache, and GPUI calls it on a hover or pressed style, a drag,
  a tooltip or a focus change (`elements/div.rs`, `Window::focus`).
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
- **Events.** `cx.emit` queues an effect; it reaches every `cx.subscribe` (or `subscribe_in`,
  which also hands the subscriber its window) when the update that emitted it ends
  (`App::flush_effects`), so what a subscriber notifies is drawn in the same next frame as the
  emitter's change (`app.rs` `Effect::Emit`, `app/context.rs` `subscribe_in`).

## How Zed shapes a window with it

Zed at
[69e2130](https://github.com/zed-industries/zed/tree/69e2130295c2649963eb639fc70b4f2ee8ea1624),
the commit 0.2.2 was published from: the workspace is a view whose panes and dock panels are
entities, each shown as `AnyView::from(pane).cached(…)` (`workspace/src/pane_group.rs`,
`dock.rs`); a terminal is a model entity fed by alacritty's thread through a channel awaited in
`cx.spawn` and batched per 4 ms, which emits `Wakeup`, and its view subscribes and notifies
(`terminal/src/terminal.rs`, `terminal_view.rs`); the grid is one custom `Element` that batches
text runs per row and takes text input through an `InputHandler` (`terminal_element.rs`); the
cursor blink is a `cx.spawn` timer loop that notifies its own view (`editor/src/blink_manager.rs`).
