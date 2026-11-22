# scenes-web.sh: the web demo's scenes (DEMO_SCENES=web), sourced by run-demo.sh, which defines at,
# k, mark, caption, zoom and cue. Leon.app ("your Mac") on the left half of the VM's screen, the
# page in kit/bin/viewer ("your laptop") on the right half (kit/web.sh). About 56 s; compose.py
# web renders all of it, web-short the WEB_CUT pieces by these marks.
SCENES=6 SECS=56
# w STEP…: kit/web.sh in the VM (setup and the take)
w() {
  cue "\"type\": \"step\", \"step\": $(jstr "web $*")"
  [ -n "${CUE_ONLY:-}" ] && { spend 0.3; return; }
  ssh $(ssh_opts) "demo@$(cat "$work/.ip")" "bash kit/web.sh $(printf "%q " "$@")"
}
# where things are, in points on the VM's 1440x810 screen (measured from VNC shots of a setup)
SERVER_DOWNS=${SERVER_DOWNS:-7}  # Settings' sidebar: Down presses from General to Server (Tailscale access, Web page)
SERVE_X=${SERVE_X:-1104} SERVE_Y=${SERVE_Y:-423} # "Let my other machines on Tailscale connect"'s switch
WEB_X=${WEB_X:-1104} WEB_Y=${WEB_Y:-589}   # "Web page"'s switch, 30 pt lower once the first is on
SETTINGS_ZOOM=${SETTINGS_ZOOM:-"1000 740 1300 880"} # raw pixels: Settings' Tailscale access and Web page
APP_SHELL_X=${APP_SHELL_X:-612} APP_SHELL_Y=${APP_SHELL_Y:-288}   # the app's shell pane
PAGE_SHELL_X=${PAGE_SHELL_X:-1310} PAGE_SHELL_Y=${PAGE_SHELL_Y:-288} # the page's shell pane
PAGE_BLOCKED_X=${PAGE_BLOCKED_X:-835} PAGE_BLOCKED_Y=${PAGE_BLOCKED_Y:-274} # the page's blocked row in Activity
PAGE_AGENT_X=${PAGE_AGENT_X:-1066} PAGE_AGENT_Y=${PAGE_AGENT_Y:-288} # the page's agent pane

scene1() { # your Mac: tests in a shell, an agent at work
  mark "1 your mac"
  at 0.5; k type "python3 runtests.py"; k key return
  at 1.4; w go
  caption "Your Mac: an agent at work." 6.6
  at 0.6; zoom 16 76 1440 1512 6.6 # Leon.app's window
}
scene2() { # Settings, Server: Tailscale access, and the web page on; its address and QR
  at 7; mark "2 settings"; k combo cmd+, # the first shortcut: lead.py measures the video's lead on it
  caption "Turn on the web page." 15.6 "Settings · Server"
  at 7.6; zoom $SETTINGS_ZOOM 15.6
  at 7.9
  for ((i = 0; i < SERVER_DOWNS; i++)); do k key down; done
  at 9.4; k click "$SERVE_X" "$SERVE_Y"
  at 10.8; k click "$WEB_X" "$WEB_Y" # the address and the QR code appear under it
  at 15.8; k key esc
}
scene3() { # your laptop: the address in a browser; the same sessions, live
  at 16.4; mark "3 laptop"; w viewer
  caption "Open it on your other machine." 23.4 "the same sessions, live"
}
scene4() { # typing both ways
  at 24; mark "4 typing"; k click "$PAGE_SHELL_X" "$PAGE_SHELL_Y"
  caption "Type from anywhere on your tailnet." 34.6
  at 24.8; k type "echo typed in the browser"; k key return
  at 29; k click "$APP_SHELL_X" "$APP_SHELL_Y"
  at 29.8; k type "echo and back on the Mac"; k key return
}
scene5() { # the agent asks; both Activity lists show it; the person answers from the browser
  at 35; mark "5 blocked"; w go ask
  at 36; k combo cmd+shift+a # the app's Activity, in its sidebar; it stays open
  caption "An agent needs you: both show it." 39.8
  at 39; k click "$PAGE_BLOCKED_X" "$PAGE_BLOCKED_Y" # the page's Activity row: its tab and pane
  caption "Answer it from the browser." 46.6 "the web page counts as a person"
  at 40.2; k click "$PAGE_AGENT_X" "$PAGE_AGENT_Y" # the keys go to the pane clicked
  at 40.9; k type y
  at 41.5; k key return
}
scene6() { # the same page, phone-sized, as if from the QR code
  at 47; mark "6 phone"; w unviewer; w viewer phone
  caption "Or from your phone." 55.6 "the QR code in Settings opens it"
}
