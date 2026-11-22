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
TAILSCALE_DOWNS=${TAILSCALE_DOWNS:-6}  # Settings' sidebar: Down presses from the first section to Tailscale
SERVE_X=${SERVE_X:-0} SERVE_Y=${SERVE_Y:-0} # "Serve over Tailscale"'s switch
WEB_X=${WEB_X:-0} WEB_Y=${WEB_Y:-0}         # "Web page"'s switch
SETTINGS_ZOOM=${SETTINGS_ZOOM:-"400 120 2080 1400"} # raw pixels: the Settings window
APP_SHELL_X=${APP_SHELL_X:-560} APP_SHELL_Y=${APP_SHELL_Y:-400}   # the app's shell pane
PAGE_SHELL_X=${PAGE_SHELL_X:-1270} PAGE_SHELL_Y=${PAGE_SHELL_Y:-400} # the page's shell pane
PAGE_BLOCKED_X=${PAGE_BLOCKED_X:-830} PAGE_BLOCKED_Y=${PAGE_BLOCKED_Y:-150} # the page's blocked row in Activity

scene1() { # your Mac: tests in a shell, an agent at work
  mark "1 your mac"
  at 0.5; k type "python3 runtests.py"; k key return # the first key: lead.py measures the video's lead on it
  at 1.4; w go
  caption "Your Mac: an agent at work." 6.6
  at 0.6; zoom 32 76 1392 1512 6.6 # Leon.app's window
}
scene2() { # Settings, Tailscale: serve over Tailscale, and the web page on; its address and QR
  at 7; mark "2 settings"; k combo cmd+,
  caption "Turn on the web page." 15.6 "Settings · Tailscale"
  at 7.6; zoom $SETTINGS_ZOOM 15.6
  at 7.9
  for ((i = 0; i < TAILSCALE_DOWNS; i++)); do k key down; done
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
  at 36; k combo cmd+shift+a # the app's Activity list
  caption "An agent needs you: both show it." 39.8
  at 38.6; k key esc
  at 39.4; k click "$PAGE_BLOCKED_X" "$PAGE_BLOCKED_Y" # the page's Activity row: its pane
  caption "Answer it from the browser." 46.6 "the web page counts as a person"
  at 40.4; k type y
  at 41; k key return
}
scene6() { # the same page, phone-sized, as if from the QR code
  at 47; mark "6 phone"; w unviewer; w viewer phone
  caption "Or from your phone." 55.6 "the QR code in Settings opens it"
}
