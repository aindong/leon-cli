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
# as ever: the viewer opens the page by its MagicDNS name, which /etc/hosts maps to 127.0.0.1.
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
name=leon-demo.example-tailnet.ts.net
app_pid() { pgrep -fx "$app/Contents/MacOS/leon" || true; }

case "${1:-}" in
  stage)
    pkill -f '[k]it/bin/tailscaled.py' 2>/dev/null || true
    nohup python3 "$kit/bin/tailscaled.py" "$sock" >/tmp/tailscaled.log 2>&1 &
    for _ in $(seq 50); do [ -S "$sock" ] && break; sleep 0.1; done
    [ -S "$sock" ] || { echo "web: the LocalAPI stand-in didn't start (/tmp/tailscaled.log)" >&2; exit 1; }
    # the made-up name, here: the clone's admin account adds the line once (its password is the
    # Tart image's own)
    grep -q " $name\$" /etc/hosts ||
      ssh devbox "echo admin | sudo -S sh -c 'echo \"127.0.0.1 $name\" >> /etc/hosts'" >/dev/null 2>&1 ||
      echo "web: no /etc/hosts line for $name: the viewer opens 127.0.0.1" >&2
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
s.update(window={"x": 16, "y": 38, "w": 696, "h": 756}, display=1)
json.dump(s, open(p, "w"))' "$d/state/app.json"
    open --env LEON_RUNTIME_DIR="$LEON_RUNTIME_DIR" --env LEON_STATE_DIR="$LEON_STATE_DIR" \
      --env LEON_CONFIG="$LEON_CONFIG" --env LEON_TAILSCALE="$LEON_TAILSCALE" \
      --env LEON_TEST_TAILNET="$LEON_TEST_TAILNET" --env PATH="$PATH" --env SHELL=/bin/zsh "$app"
    for _ in $(seq 50); do [ -n "$(app_pid)" ] && break; sleep 0.1; done
    sleep 2
    keypost park
    echo "web: staged, app pid $(app_pid)" ;;

  viewer)
    url=$(leon server status --json | python3 -c 'import json,sys; w=json.load(sys.stdin)["tailnet"]["web"]; print(w[0] if w else "")')
    [ -n "$url" ] || { echo "web: the page isn't served (Settings: Tailscale, Web page)" >&2; exit 1; }
    grep -q " $name\$" /etc/hosts || url=${url/$name/127.0.0.1}
    if [ "${2:-}" = phone ]; then geo="1000 38 400 756"; else geo="728 38 696 756"; fi
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
