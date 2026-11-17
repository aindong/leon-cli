# scenes.sh: the take's scenes, sourced by run-demo.sh, which defines at, k, g, mark, caption and zoom
scene1() { # the empty state: the lion, "Keep your work in view", the shortcuts
  mark "1 empty state" # no caption: the empty state says it
  # zoomed on its top half from before the first frame (the move ends at -1.6 s, so the video
  # opens on it): the empty state's third row names two other products. It chains into scene 2's zoom
  printf '{"t": -2.3, "type": "zoom", "region": [757, 271, 1365, 768], "until": 4.6}\n' >>"$cuefile"
}
scene2() { # build up: a session, a shell, an agent beside it, then the others one by one
  at 4; mark "2 build-up"; k combo cmd+n
  caption "Start with one terminal." 10.5
  at 5.2; zoom 0 9 1600 900 10.3 # the sidebar and the shell, as it is typed into
  at 5.5; k type "cd work/shelf-api"; k key return
  at 7.3; k type "leon split --ratio 60 -- agent"; k key return
  at 10.6; caption "Add agents as you go." 17.3
  at 11; g build 1  # a dev server tab (leon run shows it), and the session gets its name
  at 12.2; k combo cmd+1
  at 13.4; g build 2
  at 14.6; g build 3
}
scene3() { # the agent opens a pane under itself and runs the tests
  at 17.5; mark "3 agent runs its tests"; g go shelf-tests
  caption "Agents open their own panes." 25.8 "leon split · leon wait · leon capture"
  at 18.2; zoom 1470 960 1378 640 24.6 # the tests pane the agent opened
}
scene4() { # Activity: blocked, error, done; jump to the blocked one and answer
  at 26; mark "4 activity"; k combo cmd+shift+a
  caption "Working, blocked, done: at a glance." 29.4
  at 26.4; zoom 0 60 1040 560 30.2 # the Activity list
  at 28.5; k click 100 100 # the blocked row (the pointer is parked again at once)
  at 29.5; caption "Jump to the one that needs you." 36.8
  at 30; k type y
  at 30.6; k key return
  at 34; k combo cmd+shift+a
}
scene5() { # speed: a 400 MB coloured log floods the shell while the agent takes typing; 50 tabs
  at 37; mark "5 speed"; k combo cmd+k
  at 37.7; k type pagination # not "pag": status-page matches that too
  at 39.2; k key return
  at 39.8; k combo cmd+alt+left
  at 40.3; k type "cat ~/logs/app.log"; k key return
  at 41.6; mark "5 flood starts"; k combo cmd+alt+right
  caption "115 MB/s through the terminal." 45.9 "and typing keeps up"
  zoom 1000 500 1300 620 45.9 # the flood's right half and the agent's prompt
  at 42.2; k type "also test limit=0 and an offset past the end"; k key return
  at 45.5; mark "5 44 tabs"; g tabs
  at 48.5; mark "5 overview"; k combo 'cmd+shift+\'
  caption "50 tabs. Every one live." 52
  at 52; k key esc
  g untabs # the 44 background tabs go: devbox has to show in the sidebar later
}
scene6() { # ⌘K to the red agent; while it fixes the page, its repo's history in a popup
  at 52.5; mark "6 switch"; k combo cmd+k
  caption "⌘K: any session, tab or pane." 55.1
  at 53.2; k type mobile
  at 54.2; k key return # its red tests show for a second
  # init.lua's ctrl+alt+g, the recipe in docs/lua/keys.md: git log --graph in a centred popup
  at 55.3; mark "6 popup"; k combo ctrl+alt+g
  caption "A popup on one key: the repo's history." 59.8 "the same key hides it"
  at 55.6; g go status-fix # the agent fixes the page under the popup: its sidebar row turns done
  at 58.4; k combo ctrl+alt+g # hidden: the focus is back on the agent, green
  at 58.7; zoom 0 9 1714 900 60.2 # the sidebar and the agent, to the pane border
}
scene7() { # Settings (⌘,): Appearance, a theme for the whole app, live, and back
  at 60.5; mark "7 settings"; k combo cmd+,
  caption "Your colours, live." 68.8 "Settings · Appearance" # the small line's font has no ⌘
  at 61.4; k key down # the sidebar's next section: Appearance
  # macOS is light in the VM, so the live column is "When macOS is light" (the window is centred)
  at 62.6; k click 937 382 # leon-light
  at 65.4; k click 937 346 # leon-dark again
  at 67.6; k key esc # closes it; the main window has the keys again
}
scene8() { # leon do: a whole tab from one Lua action
  at 69.5; mark "8 leon do"; k combo cmd+j
  caption "Script your layouts in Lua." 76.8 "leon do review"
  at 70.5; k type "leon do review"; k key return
  at 72.4; zoom 637 9 2086 1100 76.6 # the review tab's panes: the camera spans 512 (the sidebar's border) to 2848: tests, server, live tally
}
scene9() { # Tailscale: Add Host lists the tailnet; pick devbox; its session
  at 77; mark "9 tailscale"; k combo cmd+shift+p
  caption "Pick a machine from your tailnet." 84.8
  at 77.8; k type "add host"
  at 79; k key return
  # the sidebar, the palette and the shell pane: its edges fall on pane borders and column gaps
  at 79.4; zoom 0 9 1768 990 87.0
  at 81.5; k type dev
  at 82.5; k key return
  at 85; caption "Its sessions, beside your own." 90.6
  at 85.5; k combo cmd+k
  at 86.2; k type nightly
  at 87.2; k key return
  at 87.6; zoom 0 9 1714 720 90.6 # devbox in the sidebar, its agent with the devbox badge, to the pane border
}
scene10() { # leon attach in a plain terminal; an agent elsewhere asks: its banner takes you there
  at 91; mark "10 attach"; g banner # on now: the app has it well before the agent asks
  g front # the Terminal setup left on the desktop: macOS slides there
  caption "The same sessions, from any terminal." 96.3 "leon attach, in macOS Terminal"
  at 92; k type "leon attach shelf-api"; k key return # the session's tab: its agent and shell, live
  at 94.3; k type "then update the README"; k key return # to its agent: the app has it too
  at 96.5; mark "10 banner"; g go banner # logtally's agent asks again; leon's banner (the app is behind)
  caption "It tells you when an agent needs you." 100.8 "click it to go straight there"
  at 96.8; zoom 1400 9 1448 815 98.6 # the banner, top right
  at 98.4; k click 1250 78 # the banner: leon comes forward on that agent's pane
  at 99.8; k type y; k key return
  g unterminal # behind the app: after ⌘Q the desktop is bare again
}
scene11() { # quit the app; the programs keep going; open it again
  at 101.5; mark "11 quit"; k combo cmd+q
  caption "Quit the app…" 103.7 # cut.py keeps 2 s of desktop: the caption runs to the cut
  at 103.5; mark "11 reopen"; g app
  g fullscreen # it opens windowed: the compositor and take-cut.mp4 drop that second
  at 107.4; caption "…everything is still running." 111.8
  at 107.5; k combo cmd+k
  at 108.2; k type review
  at 109; k key return
}
scene12() { # the payoff: every tab, This Mac and devbox, live
  at 112; mark "12 overview"; k combo 'cmd+shift+\' # live cards: screencapture -V needs frames to stop
  caption "Keep your work in view." 115
}
