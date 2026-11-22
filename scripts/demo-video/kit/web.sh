#!/bin/bash
# web.sh STEP: the web demo inside the VM, run as `demo` over ssh by run-demo.sh (DEMO_SCENES=web).
# The launch demo's guest.sh does the rest (reset, prefs, key): this adds the web page's parts.
#   stage        the made-up tailnet (tailscaled.py), a certificate for its name, Tailscale access
#                on, leon's server with one session (shelf-api's agent, which opens its tests pane),
#                and the app on the left half, in a big font
#   viewer [phone]  "your other machine": kit/bin/viewer's browser on the right half, its address
#                bar empty for the take to paste the address Settings copied; phone: a phone with
#                the page open
#   unviewer     close the viewer
#   go NAME      let the agent go on: its start (no NAME) or a named beat (ask)
#   stop         the viewer, the proxy and the stand-in gone (guest.sh reset does the app and the servers)
# The browser must be another machine (docs/architecture/web.md: admit refuses this one). The
# demo's Leon.app is built with leon's debug-only seam (run-demo.sh webapp), LEON_TEST_TAILNET,
# which points it at tailscaled.py: it answers every whois with "laptop", another of the owner's
# machines, and lets 127.0.0.1 stand in for the tailnet. Host, Origin and the token are checked
# as ever: the viewer opens the page by its made-up MagicDNS name (studio-mac.example-tailnet.ts.net,
# from tailscaled.py) through kit/bin/proxy.py, which the VM needs because it can't resolve that
# name (/etc/hosts needs root); the proxy passes each request on unchanged to leon's port here.
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
name=studio-mac.example-tailnet.ts.net # tailscaled.py's
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
    rm -f "$sock" # the last stage's: the wait below is then for the new one
    nohup python3 "$kit/bin/tailscaled.py" "$sock" >/tmp/tailscaled.log 2>&1 &
    for _ in $(seq 50); do [ -S "$sock" ] && break; sleep 0.1; done
    [ -S "$sock" ] || { echo "web: the LocalAPI stand-in didn't start (/tmp/tailscaled.log)" >&2; exit 1; }
    pbcopy </dev/null # the take copies the address from Settings: nothing from before
    # HTTPS, as with `tailscale cert`: a certificate for the made-up name alone, made here, never
    # kept (guest.sh reset empties $d); the viewer trusts it for that name only
    mkdir -m 700 -p "$d/certs"
    /usr/bin/openssl req -x509 -newkey rsa:2048 -nodes -days 7 -subj "/CN=$name" -addext "subjectAltName=DNS:$name" \
      -keyout "$d/certs/web.key" -out "$d/certs/web.crt" 2>/dev/null
    chmod 600 "$d/certs/web.key"
    leon config set serve_web_cert "$d/certs/web.crt" >/dev/null
    leon config set serve_web_key "$d/certs/web.key" >/dev/null
    # Tailscale access is on already: the take turns on the web page
    leon config set serve_tailscale true >/dev/null
    leon server start --restore=false >/dev/null
    w=$HOME/work
    leon new shelf-api -c "$w/shelf-api" >/dev/null
    leon -s shelf-api run --keep-open -c "$w/shelf-api" -- agent "$kit/scenes/web.scene" >/dev/null
    leon -s shelf-api window close 1 >/dev/null
    # the app on the left half, the same frame every launch
    python3 -c '
import json, os, sys
p = sys.argv[1]
s = json.load(open(p)) if os.path.exists(p) else {"tab_style": "sidebar", "pane_headers": True, "sidebar_width": 170}
s.update(window={"x": 24, "y": 40, "w": 920, "h": 950}, display=1)  # the left half of 1920x1080 pt
json.dump(s, open(p, "w"))' "$d/state/app.json"
    open --env LEON_RUNTIME_DIR="$LEON_RUNTIME_DIR" --env LEON_STATE_DIR="$LEON_STATE_DIR" \
      --env LEON_CONFIG="$LEON_CONFIG" --env LEON_TAILSCALE="$LEON_TAILSCALE" \
      --env LEON_TEST_TAILNET="$LEON_TEST_TAILNET" --env PATH="$PATH" --env SHELL=/bin/zsh "$app"
    for _ in $(seq 50); do [ -n "$(app_pid)" ] && break; sleep 0.1; done
    sleep 2
    # macOS says "App Running in Background" for a new Leon.app, and the banner stays: its close
    # button shows on hover (top right of the screen); then the keys go to the app's agent pane
    keypost move 1700 80; sleep 0.8; keypost click 1564 48; sleep 0.5
    open "$app" # in front again (Finder was, for its ⌥⌘W)
    sleep 1
    keypost click 560 300
    keypost combo cmd+shift+a # Activity in the sidebar, before the take: too small a change for lead.py
    echo "web: staged, app pid $(app_pid)" ;;

  viewer)
    # the first address Settings shows: the MagicDNS name's, then the port leon serves it on
    read -r url port < <(leon server status --json | python3 -c 'import json,sys; t=json.load(sys.stdin)["tailnet"]; print((t["web"] or [""])[0], t["port"])')
    [[ "$url" == http*://*.ts.net:* ]] || { echo "web: the page isn't served by its MagicDNS name (Settings: Server, Web page)" >&2; exit 1; }
    host=${url#*://}; host=${host%%:*}
    pkill -f '[k]it/bin/proxy.py' 2>/dev/null || true
    nohup python3 "$kit/bin/proxy.py" 7399 "$host" "$port" >/tmp/proxy.log 2>&1 &
    sleep 0.5
    # beside the app (viewer.swift zooms the page to fit); the browser starts blank, for the take
    # to paste the address into; the phone opens it, as if from the QR code
    if [ "${2:-}" = phone ]; then geo="1230 60 420 880"; how=phone; else geo="968 40 928 950"; how=laptop; fi
    nohup "$kit/bin/viewer" "$url" $geo 7399 $how >/tmp/viewer.log 2>&1 &
    sleep 2
    keypost park
    echo "web: viewer at $url" ;;

  unviewer) pkill -f '[k]it/bin/viewer' 2>/dev/null || true ;;

  go) touch "$kit/go${2:+.$2}" ;;

  stop)
    pkill -f '[k]it/bin/viewer' 2>/dev/null || true
    pkill -f '[k]it/bin/tailscaled.py' 2>/dev/null || true
    pkill -f '[k]it/bin/proxy.py' 2>/dev/null || true
    rm -f "$kit"/go* ;;

  *) sed -n '2,10s/^# \{0,1\}//p' "$0"; exit 2 ;;
esac
