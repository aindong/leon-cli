#!/bin/bash
# web.sh STEP: the web demo inside the VM, run as `demo` over ssh by run-demo.sh (DEMO_SCENES=web).
# The launch demo's guest.sh does the rest (reset, prefs, key): this adds the web page's parts.
#   stage        the made-up tailnet (tailscaled.py), leon's server with two sessions (shelf-api's
#                agent and a shell, status-page's watcher), and the app on the left half
#   viewer [phone]  "your laptop": the page in kit/bin/viewer on the right half (phone: phone-sized),
#                at the first address Settings shows (server.status's tailnet.web)
#   unviewer     close the viewer
#   go NAME      let the agent go on: its start (no NAME) or a named beat (ask)
#   stop         the viewer and the stand-in gone (guest.sh reset does the app and the servers)
# The browser must be another machine (docs/architecture/web.md: admit refuses this one). The
# demo's Leon.app is built with leon's debug-only seam (run-demo.sh webapp), LEON_TEST_TAILNET,
# which points it at tailscaled.py: it answers every whois with "laptop", another of the owner's
# machines, and lets 127.0.0.1 stand in for the tailnet. Host, Origin and the token are checked
# as ever: the viewer opens 127.0.0.1, an address the server answers to (Settings shows the
# MagicDNS name; the VM can't resolve it without root).
set -euo pipefail
kit=$HOME/kit
d=$HOME/leon-demo
sock=$HOME/leon-demo-ts/tailscaled.sock # outside $d: guest.sh reset empties that
export LEON_RUNTIME_DIR=$d/run LEON_STATE_DIR=$d/state LEON_CONFIG=$d/init.lua
export LEON_TAILSCALE=$kit/bin/tailscale
export LEON_TEST_TAILNET="{\"localapi\": \"$sock\", \"loopback\": true}"
export PATH=$kit/bin:$HOME/python/bin:$HOME/leon/target/release/Leon.app/Contents/MacOS:/usr/bin:/bin:/usr/sbin:/sbin
export SHELL=/bin/zsh
app=$HOME/leon/target/release/Leon.app
app_pid() { pgrep -fx "$app/Contents/MacOS/leon" || true; }

case "${1:-}" in
  stage)
    # a clone of the shared VM can restore another Leon.app at login (its window and its server)
    # and Finder's windows: neither belongs in the shot
    for p in $(pgrep -f 'Leon.app/Contents/MacOS/leon' || true); do
      [[ "$(ps -o command= -p "$p")" == "$app/"* ]] || kill "$p" 2>/dev/null || true
    done
    # a click on the wallpaper must not reveal the desktop (it hides every window and shows the
    # widgets): the banner's close button below is clicked whether or not the banner came
    if [ "$(defaults read com.apple.WindowManager EnableStandardClickToShowDesktop 2>/dev/null)" != 0 ]; then
      defaults write com.apple.WindowManager EnableStandardClickToShowDesktop -bool false
      killall WindowManager 2>/dev/null || true
      sleep 1
    fi
    # (ssh may not send Finder Apple events: its own ⌥⌘W closes every window, keys are allowed)
    open -a Finder && sleep 1 && keypost combo cmd+alt+w && sleep 0.5
    pkill -f '[k]it/bin/tailscaled.py' 2>/dev/null || true
    nohup python3 "$kit/bin/tailscaled.py" "$sock" >/tmp/tailscaled.log 2>&1 &
    for _ in $(seq 50); do [ -S "$sock" ] && break; sleep 0.1; done
    [ -S "$sock" ] || { echo "web: the LocalAPI stand-in didn't start (/tmp/tailscaled.log)" >&2; exit 1; }
    leon config set font.size 13 >/dev/null # two windows side by side: half the screen each
    leon server start --restore=false >/dev/null
    w=$HOME/work
    leon new shelf-api -c "$w/shelf-api" >/dev/null
    a=$(leon -s shelf-api run --keep-open -c "$w/shelf-api" -- agent "$kit/scenes/web.scene")
    leon -s shelf-api split -b "$a" --split right --ratio 45 -c "$w/shelf-api" -- zsh -l >/dev/null
    leon -s shelf-api window close 1 >/dev/null
    leon new status-page -c "$w/status-page" >/dev/null
    leon -s status-page run --keep-open -c "$w/status-page" -- sh -c 'printf "\033]2;watch :5173\007"; exec python3 -u watch.py' >/dev/null
    leon -s status-page window close 1 >/dev/null
    # the app on the left half, the same frame every launch
    python3 -c '
import json, os, sys
p = sys.argv[1]
s = json.load(open(p)) if os.path.exists(p) else {"tab_style": "sidebar", "pane_headers": True, "sidebar_width": 200}
s.update(window={"x": 8, "y": 38, "w": 720, "h": 756}, display=1)  # 720: its narrowest
json.dump(s, open(p, "w"))' "$d/state/app.json"
    open --env LEON_RUNTIME_DIR="$LEON_RUNTIME_DIR" --env LEON_STATE_DIR="$LEON_STATE_DIR" \
      --env LEON_CONFIG="$LEON_CONFIG" --env LEON_TAILSCALE="$LEON_TAILSCALE" \
      --env LEON_TEST_TAILNET="$LEON_TEST_TAILNET" --env PATH="$PATH" --env SHELL=/bin/zsh "$app"
    for _ in $(seq 50); do [ -n "$(app_pid)" ] && break; sleep 0.1; done
    sleep 2
    # macOS says "App Running in Background" for a new Leon.app, and the banner stays: its close
    # button shows on hover (top right of the screen); then the keys go to the app's shell
    keypost move 1200 80; sleep 0.8; keypost click 1084 48; sleep 0.5
    open "$app" # in front again (Finder was, for its ⌥⌘W)
    sleep 1
    keypost click 612 288
    echo "web: staged, app pid $(app_pid)" ;;

  viewer)
    # the address on 127.0.0.1 (the VM can't resolve the made-up MagicDNS name without root)
    url=$(leon server status --json | python3 -c 'import json,sys; print(next((u for u in json.load(sys.stdin)["tailnet"]["web"] if "//127.0.0.1:" in u), ""))')
    [ -n "$url" ] || { echo "web: the page isn't served (Settings: Tailscale, Web page)" >&2; exit 1; }
    # the page beside the app, 728 high under its title bar (viewer.swift zooms it to fit)
    if [ "${2:-}" = phone ]; then geo="1000 38 400 728"; else geo="736 38 688 728"; fi
    nohup "$kit/bin/viewer" "$url" $geo >/tmp/viewer.log 2>&1 &
    sleep 2
    keypost park
    echo "web: viewer at $url" ;;

  unviewer) pkill -f '[k]it/bin/viewer' 2>/dev/null || true ;;

  go) touch "$kit/go${2:+.$2}" ;;

  stop)
    pkill -f '[k]it/bin/viewer' 2>/dev/null || true
    pkill -f '[k]it/bin/tailscaled.py' 2>/dev/null || true
    rm -f "$kit"/go* ;;

  *) sed -n '2,10s/^# \{0,1\}//p' "$0"; exit 2 ;;
esac
