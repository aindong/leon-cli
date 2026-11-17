# Architecture: switching and attention

The session switcher, the Activity list, notifications and the Dock badge, and `leon notify`. Part
of [the architecture](../architecture.md).

## Decisions

- **Switcher and attention in the app.** The session switcher (`session.switch`, ⌘K) uses the
  palette's box over `switcher::rank`: every tab of every connected machine as `host › session ›
  tab` with its status glyph, plus a block when the query matches the title its program set; no
  query lists the tabs most recently shown first (the window ids the app saw on screen), a query
  runs the palette's fuzzy filter, and among equal matches what needs the user (blocked, error,
  done) comes first, then the more recent. ↩ goes there (`session.focus_block`
  or `session.focus_window` on that machine); ⌘↩ moves the tab's shown block beside the focused
  pane with `session.move_block` when both are on the same machine (from another session it
  joins this one, tmux's `join-pane`; its program keeps its `LEON_SESSION`, so leon commands in it
  follow `LEON_BLOCK`; the session it leaves gets `block_closed`, the one it joins
  `block_created`; a floating focus anchors on a tiled pane, and a zoomed window unzooms). On a
  row of another machine the switcher stays open and says a pane can't join a window on a different machine. Activity
  (`client.sidebar.activity.toggle`, ⌘⇧A, a toolbar button) takes the sidebar's place, shown even
  in top style or with the sidebar hidden. `attention::Tracker` looks at every machine's model at
  each poll: each block with a status becomes a row, blocked (its kind), error, done (unseen:
  arriving at the pane sends the `focus` event that clears it on the server), working (its
  progress), the longest-waiting first in a state, waiting since the app first saw that state; a
  machine's first look reports no changes. The toolbar button and the Dock badge show the blocked
  count, nothing at 0. `attention::Notifier` turns changes into notices: entering blocked or error
  (not for a block on screen while the window is key), or done after at least 30 s of working while
  the app is in the background; one per block and state a minute and four a minute in all, the rest
  folded into one "N more" notice later; `leon.set("notifications.blocked" |
  "notifications.error" | "notifications.done", false)` turns a kind off (carried in `client.keymap`'s settings). Text is the
  host › session › tab names and the record's caption with OSC 7501's hidden characters (`strip_hidden`) and line separators removed, the caption
  cut first so the state always shows,
  at most 120 chars. Notices go through `UNUserNotificationCenter` (`objc2`, already in GPUI's tree),
  which applies Do Not Disturb and Focus itself; only a bundled app has the main-bundle identifier it
  needs, so a bare `leon app` binary logs once and posts nothing. The center's delegate queues a
  clicked notice's host label, session and block and wakes every window's poll; the window that knows the pane goes there (the folded
  notice opens Activity).
- **The overview and hover previews** (`overview.rs`, `view_overview.rs`).
  `client.overview.toggle` (⌘⇧\, the toolbar's grid button) and `client.overview.session.toggle`
  (⌘\: the selected session only) take the whole window: a card per tab of every machine, grouped
  under its session and, once there are hosts, its machine, in the sidebar's order. Each card has
  the tab's title, the sidebar's status (`overview::tab_statuses`, which the sidebar's rows use too)
  in Activity's words, and a thumbnail. `overview::fit` places the cards in the pane area's shape:
  a session's cards are a block as wide as its cards, blocks sit side by side while they fit, and
  the cards take the widest size, 640 px down to 240 px, at which everything fits the window's
  height (past that the grid scrolls). `overview::step` moves the arrow keys by order and by the
  nearest column. ↩ or a click is `go_to`, esc changes nothing, and typing runs the palette's
  filter over the titles. A machine that isn't connected shows its last model's cards under a grey
  veil, with `HostState::note` as the reason. A machine never reached shows only its label.
  A thumbnail is a cached GPUI view (`AnyView::cached`) of the tab's tiles (`panes::tiles`, zoom
  honoured; floats and drawers are left out), painted with `paint_frame` at a scaled cell and font
  from the frames the server pushes: never faded, never fitted, so no PTY is resized. While a
  thumbnail is up its blocks are in its machine's watch set (`watch_screens`): the hovered tab's
  first, then the cards from the selected one on, up to the 256 a server watches (past that the
  last cards get no thumbnail). The poll refreshes
  one only when its screens, layout, theme or font changed, at most every 250 ms (`THUMB_EVERY`),
  and gives it a new view, which draws, while every other card's view replays its cached paint
  (a new view, not a notify of the old one: on CI's runner a notified cached view was not always
  drawn again). `signature()` leaves screens out (a pushed
  screen redraws only its pane's grid, `view_grid.rs`), so a busy pane's screen never redraws
  the window on its own, and an idle
  overview draws nothing. Opening moves the
  current tab's card from the pane area to its place over a drawer's slide (`floats::slide`,
  120 ms), and closing moves the chosen card back. The rest of the grid shows through a veil drawn
  over the thumbnails, never as their opacity, since a cached view replays its paint as recorded.
  Reduce Motion skips both. Resting on a tab in the titlebar or a tab row in the sidebar for
  400 ms (`HOVER_DELAY`) shows the same thumbnail in a hover preview under the tab or beside the
  row. Entering another tab within 150 ms of leaving one moves the preview at once, and leaving
  hides it. `LEON_FRAME_STATS` says `leon overview: open`, and `LEON_FRAME_LOG` logs the opening's
  first frame and the first frame with every thumbnail's screen (`overview`, `overview-filled`).
- **Fading the panes without the focus.** With two or more panes on screen the app draws every
  pane but the focused one (`floats::target`, so a focused float or drawer counts) blended toward
  the window's background, header included: `fade::theme` moves each colour of the theme toward
  `chrome`, and `fade::frame` does the same to the colours of the pane's screen before it is
  painted. The four status colours are left out, so an indicator in a faded pane keeps its
  colour, and a pane whose own indicator is blocked, error or done (what needs the user, above)
  is not faded at all. Blocked holds however long the user looked at it; a done or error that
  came while the pane had the focus in the key window was seen, so it fades once the user leaves
  (the server keeps it until they come back, for Activity and the badge), and any change while
  they are away needs them again. A pane alone
  on screen is not faded, nor is anything while the focus is on a pane that isn't shown. The
  sidebar, the tabs and the titlebar are not panes and never fade, and the fade doesn't read
  whether the window is the key window. A pane's fade moves to a new target from where it is,
  over a drawer's slide (`floats::SLIDE`, 120 ms, eased out; at once with Reduce Motion), so a
  focus that moves on mid-fade never jumps; a pane seen for the first time is drawn in place.
  The poll redraws only while a fade is on its way (`Workspace::fading`), so an idle app still
  draws nothing. `leon.set("fade_inactive_panes", N)` is the strength, 0 (off) to 0.8; the
  default, 0.35, is the strongest step at which body text keeps 4.5:1 on every built-in theme.
  The app reads it from `server.settings`. `leon attach` doesn't fade.
- **The translucent window.** `leon.set("translucency", N)` (0 to 0.8, default 0.3) asks GPUI for
  a blurred window background (`Window::set_background_appearance`, an `NSVisualEffectView`
  under the content that the system composites, so a desktop changing behind the window draws
  nothing in leon; leon sets the view's material, or macOS 27 doesn't blur: `view_glass.rs` in
  gpui.md's Allowed outside GPUI) and draws the window's own colours with alpha (`glass.rs`).
  GPUI's renderer adds alpha where fills overlap, so each pixel gets one translucent fill at most: the root fills
  nothing, and the sidebar, the title bar, the panes area's gaps (`glass::gaps`, the area less
  the rounded tiles) and each tile fill themselves. Each alpha is what the setting asks, or
  more where text would read worse over some desktop than it must (white, black, or the grey
  between them that matches the text): body text on a pane 4.5:1, a pane header's `muted`
  caption 3:1, `text` on the chrome 4.5:1 and `muted` 3:1, each capped at what the theme gives
  it opaque, so an imported low-contrast theme is never made worse. The maths lets
  a quarter more of the desktop through than the alpha says (`SEE_GAIN`: measured on screen, the
  window server shows about a fifth more). Every pane gets one alpha, enough for the theme
  faded by `fade_inactive_panes` too, so a faded pane keeps its fade's contrast and the focused
  pane is never the paler one; with the default fade the panes stay nearly opaque (0.95 on
  leon-dark) and the chrome carries the translucency (0.82 on leon-dark, 0.79 on leon-light,
  where the floor stops each from 0.2 up). An inactive window keeps its blur (GPUI's effect view stays
  active). The window is opaque, exactly as before, in full screen, with Reduce Transparency on
  (read when the window becomes active), at 0, and until the server's settings first arrive.
  Floats, a drawer sliding out (over tiles that took its room back) and overlays (the palette,
  the overview, the find bar) stay opaque: they sit over other content. A program's own background colours are
  opaque; only default-background cells show through.
- **A theme change fades in.** A new theme (Change Theme…, Settings, init.lua, the appearance)
  moves every colour from the old theme's to the new one's over the same slide time, and every
  pane, one that needs the user too, dips toward the window mid-way (at most 0.8), so its text,
  which the server sends again in the new colours, fades out and back in. `render` asks GPUI for
  animation frames only while it moves (ADR R6); Reduce Motion, and the first settings after a
  launch, switch at once. The theme list's preview is drawn the same way.
- **An icon per program** (`program_kind.rs`). Each pane header, and each tab, sidebar row and
  overview card for the pane its tab shows (`Session::named_tile`), draws a glyph for the kind of
  program running there, before its title; the status mark keeps its place and colour. The
  server reports the program as `foreground` in `session.list_blocks`: the name of the PTY's
  foreground process group's leader (`tcgetpgrp`, then `proc_pidinfo` or `/proc`), or a live
  member's when a pipeline's first process is gone. Only its name, never its arguments, and
  null for another user's process. Its 200 ms tick sends `foreground_changed` on a change, so
  the app redraws through the refetch it already does for any event. One table maps names to
  kinds. A name it doesn't know (an interpreter: a `#!` script reads as one) is looked up by the
  pane title's first word, which shells set to the running command; else it is an agent when the
  block has an OSC 7501 report (a record an agent leaves outlives it), else generic. A server without the field
  falls back to argv0.
- **`leon notify`.** `dev.leon.terminal.report_status {state, title, msg}` (experimental) builds an
  OSC 7501 report (title and msg base64) and runs it through the same parser and store as PTY
  output, so every limit applies; text the parser would drop is refused (400) instead. `leon notify`
  calls it for `-b`, else `$LEON_BLOCK`, never the focused pane (exit 2 with neither), since a job
  outside leon would light up whatever the user looks at. It writes no escape to a tty: agents' tool
  shells and launchd jobs have none.
