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
  default, 0.35, is the strongest step at which body text keeps 4.5:1 on leon-dark and
  leon-light. The app reads it from `server.settings`. `leon attach` doesn't fade.
- **`leon notify`.** `dev.leon.terminal.report_status {state, title, msg}` (experimental) builds an
  OSC 7501 report (title and msg base64) and runs it through the same parser and store as PTY
  output, so every limit applies; text the parser would drop is refused (400) instead. `leon notify`
  calls it for `-b`, else `$LEON_BLOCK`, never the focused pane (exit 2 with neither), since a job
  outside leon would light up whatever the user looks at. It writes no escape to a tty: agents' tool
  shells and launchd jobs have none.
