# scenes-web.sh: the web demo's scenes (DEMO_SCENES=web), sourced by run-demo.sh, which defines at,
# k, mark, caption, zoom and cue. "Open it anywhere": Leon.app ("your Mac") on the left half of the
# VM's screen; Settings copies the page's address; a browser on "another machine" (kit/bin/viewer,
# the right half) has it pasted and shows the same session; the agent is answered from it, then
# from a phone. About 43 s; compose.py web renders all of it, the short cuts the WEB_CUT pieces by
# these marks (compose/web.py).
SCENES=5 SECS=43 SRC=3840x2160 # the clone's screen: 1920x1080 pt at 2x (README: tart set --display)
# w STEP…: kit/web.sh in the VM (setup and the take)
w() {
  cue "\"type\": \"step\", \"step\": $(jstr "web $*")"
  [ -n "${CUE_ONLY:-}" ] && { spend 0.3; return; }
  ssh $(ssh_opts) "demo@$(cat "$work/.ip")" "bash kit/web.sh $(printf "%q " "$@")"
}
# where things are, in points on the clone's 1920x1080 screen (measured from VNC shots of a setup)
SERVER_DOWNS=${SERVER_DOWNS:-7}  # Settings' sidebar: Down presses from General to Server
WEB_X=${WEB_X:-1344} WEB_Y=${WEB_Y:-738}   # "Web page"'s switch (Tailscale access is on already)
SCROLL_DY=${SCROLL_DY:--200}               # Settings' content scrolled to its end: the whole QR code
COPY_X=${COPY_X:-1162} COPY_Y=${COPY_Y:-613} # the address's Copy button, after that scroll
PAGE_BLOCKED_X=${PAGE_BLOCKED_X:-1080} PAGE_BLOCKED_Y=${PAGE_BLOCKED_Y:-318} # the browser's blocked row in Activity
PAGE_AGENT_X=${PAGE_AGENT_X:-1500} PAGE_AGENT_Y=${PAGE_AGENT_Y:-495}       # the browser's agent pane
PHONE_BLOCKED_X=${PHONE_BLOCKED_X:-1390} PHONE_BLOCKED_Y=${PHONE_BLOCKED_Y:-278} # the phone's blocked row
PHONE_PANE_X=${PHONE_PANE_X:-1440} PHONE_PANE_Y=${PHONE_PANE_Y:-525}       # the phone's pane, once open
# zooms, in the 3840x2160 take's pixels
APP="48 80 1840 1900" SETTINGS_ZOOM="1500 1000 1260 560" WEB_ZOOM="1490 1050 1240 620"
BROWSER="1936 60 1856 1900" BROWSER_TOP="1936 60 1856 700" PHONE="2440 100 880 1800"

scene1() { # your Mac: an agent at work, Activity open (kit/web.sh stage opens it)
  mark "1 your mac"
  at 0.6; w go
  caption "Your Mac: an agent at work." 5.8
  at 0.6; zoom $APP 5.8 # Leon.app's window
}
scene2() { # Settings, Server: the web page on; its address and QR code; Copy
  at 6; mark "2 settings"; k combo cmd+, # the first shortcut: lead.py measures the video's lead on it
  caption "Turn on the web page." 14.4 "Settings · Server"
  at 6.6; zoom $SETTINGS_ZOOM 11.8
  at 6.8
  for ((i = 0; i < SERVER_DOWNS; i++)); do k key down; done
  at 7.8; k click "$WEB_X" "$WEB_Y" # the address and the QR code appear under it, within 5 s
  at 12; k scroll 1050 675 "$SCROLL_DY"; zoom $WEB_ZOOM 14.4
  at 12.8; k click "$COPY_X" "$COPY_Y" # the address, copied
  at 14.2; k key esc
}
scene3() { # a browser on another machine: the address pasted, Return, the same session, live
  at 14.6; mark "3 browser"; w viewer # its address bar empty and focused
  caption "Open it anywhere: paste it into a browser." 23
  at 16.8; zoom $BROWSER_TOP 19.4 # the address bar
  at 17.6; k combo cmd+v
  at 18.6; k key return
  at 19.6; zoom $BROWSER 23 # the session appearing, held
}
scene4() { # the agent asks; the browser answers it
  at 23; mark "4 answer"; w go ask
  caption "Answer your agents from it." 29.6 "the web page counts as a person"
  at 24.2; k click "$PAGE_BLOCKED_X" "$PAGE_BLOCKED_Y" # the page's Activity row: its pane
  at 25; k click "$PAGE_AGENT_X" "$PAGE_AGENT_Y" # the keys go to the pane clicked
  at 25.6; k type y
  at 26.1; k key return
}
scene5() { # anywhere: a phone, as if from the QR code; it answers the agent's next question
  at 30; mark "5 phone"; w unviewer; w viewer phone
  w go ask2
  caption "Or from your phone." 42.6 "the QR code in Settings opens it"
  at 32.4; zoom $PHONE 42.6
  at 35.4; k click "$PHONE_BLOCKED_X" "$PHONE_BLOCKED_Y" # Activity's row: its pane, the key row under it
  at 36.6; k click "$PHONE_PANE_X" "$PHONE_PANE_Y"
  at 37.2; k type y
  at 37.8; k key return
}
