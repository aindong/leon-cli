# Scrollback: alacritty's window and leon's store

How a terminal keeps its history (`crates/leon-core/src/terminal_history.rs`). The numbering
clients see is in [Protocol and clients](protocol-and-clients.md), "Scrollback in the app".

- **Why a store.** alacritty keeps every history row at the pane's full width, 24 bytes a cell:
  10,000 lines at 80 columns hold 19.7 MB whatever they say, 48 MB at 200 columns. leon keeps
  alacritty's history to `WINDOW` (1,000) rows and moves older rows into a store of its own, as
  their characters and runs of style (colours and flags, 12 bytes a run), with alacritty's own
  `CellExtra` (combining marks, underline colour, OSC 8 link) kept for the few cells that have one.
  A line of 70 characters with a few colours costs about 170 bytes instead of 1,920. 10,000 such
  lines cost 3.5 MB in all (`b1_store_memory.rs` counts it). Without a fork: rows are read through
  `Term::grid()`, and `Grid::update_history` drops them from alacritty.
- **Why nothing is lost on the way.** alacritty drops its oldest row when a push finds its history
  full. So output is fed in pieces that can't push more rows than the window has room for: a row is
  pushed by a line feed (LF, VT, FF, `ESC D`, `ESC E`) or by text wrapping past the last column, so
  a piece holds at most half its room in line feeds and `cols` × the other half in bytes. Before
  each piece on the primary screen, alacritty's rows past 800 move into the store, which keeps 200
  rows of room: half of it for whatever a piece that started on the alternate screen pushes after
  switching back. `CSI n S` and a delete-line on the top row push up to a screen at once; a piece of
  them can overflow the window, which `feed` sees, and then it moves the count by `GAP` (past any
  number a client keeps), since the store's lines would otherwise be numbered one too far.
- **Numbering.** The store's lines are numbered just above alacritty's oldest: line `scrolled -
  history` is the store's oldest, and `history` counts both. A row keeps its number as it moves, so
  nothing a client holds (its cache, a selection, a find result) changes. `lines` reads a range
  across both; `text` (`capture`, persistence, `leon attach`) is the store's text then alacritty's.
- **The alternate screen.** It has no history, as before: nothing moves while it shows (alacritty's
  primary grid is out of reach then), `history` is 0 and `text` is that screen alone.
- **Clears.** `ESC c` resets both screens, so it clears the store. `CSI 3 J` clears only the history
  of the screen showing, so it clears the store only when its piece ran on the primary screen.
- **Resize.** alacritty re-wraps its own rows; the store's are never re-wrapped. A narrower pane
  cuts a stored row at its edge in `lines` (`text` keeps it whole); a wider one shows it as it was.
  Before a re-wrap on the primary screen alacritty's limit is raised, so the re-wrap drops nothing,
  and what spills past the window moves into the store. On the alternate screen the primary grid
  re-wraps out of reach, keeping 1,000 rows (known limit).
- **The cap.** `leon.set("scrollback", N)` (0 to 100,000, default 10,000) caps the store and
  alacritty's rows together. The server applies it to every terminal on a reload and to each new
  one, and a lower value drops the oldest lines at once.
- **Known limits.** Find, selection and links read alacritty's rows only, so they reach the newest
  1,000 lines of history, not the store's. A stored tab is shown in `text` with alacritty's default
  8-column stops (a program's own, set with HTS, aren't readable from outside). The underline colour
  is kept but not drawn, as before.
