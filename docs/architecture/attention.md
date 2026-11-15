# Architecture: switching and attention

The session switcher, the Activity list, notifications and the Dock badge, and `leon notify`. Part
of [the architecture](../architecture.md).

## Decisions

- **Switcher and attention in the app.** The session switcher (`session.switch`, ⌘K) uses the
  palette's box over `switcher::rank`: every tab of every connected machine as `host › session ›
  tab` with its status glyph, plus a block when the query matches the title its program set; no
  query lists the tabs most recently shown first (the window ids the app saw on screen), a query
  runs the palette's fuzzy filter with recency breaking ties. ↩ goes there (`session.focus_block`
  or `session.focus_window` on that machine); ⌘↩ moves the tab's shown block beside the focused
  pane with `session.move_block` when both are on the same machine (from another session it
  joins this one, tmux's `join-pane`; its program keeps its `LEON_SESSION`; a floating focus anchors on
  a tiled pane, and a zoomed window unzooms). On a row of another
  machine the switcher stays open and says a pane can't join a window on a different machine. Activity
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
  clicked notice's host label, session and block and wakes the poll, which goes there (the folded
  notice opens Activity).
- **`leon notify`.** `dev.leon.terminal.report_status {state, title, msg}` (experimental) builds an
  OSC 7501 report (title and msg base64) and runs it through the same parser and store as PTY
  output, so every limit applies; text the parser would drop is refused (400) instead. `leon notify`
  calls it for `-b`, else `$LEON_BLOCK`, never the focused pane (exit 2 with neither), since a job
  outside leon would light up whatever the user looks at. It writes no escape to a tty: agents' tool
  shells and launchd jobs have none.
