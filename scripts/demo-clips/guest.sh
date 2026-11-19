#!/bin/bash
# guest.sh STEP SCENE: demo-clips.sh's scenes inside the demo VM, run as `demo` over ssh. It uses
# the launch video's kit there (~/kit: keypost, git, the stand-in tailscale) and the app that
# `run-demo.sh push` put in ~/leon; its own leon dirs are ~/leon-clips, never the take's. The app is
# the take's too: don't record clips while a launch video take runs in the same VM.
#   stage SCENE  quit the app, a fresh server with the scene's sessions, the app windowed at
#                WINDOW (points), pointer parked
#   play SCENE   the scene's keys and clicks (keypost, inside the VM)
#   stop         quit the app and stop the server
set -euo pipefail
kit=$HOME/kit
clips=$HOME/clips
d=$HOME/leon-clips
app=$HOME/leon/target/release/Leon.app
export LEON_RUNTIME_DIR=$d/run LEON_STATE_DIR=$d/state LEON_CONFIG=$d/init.lua
export LEON_TAILSCALE=$kit/bin/tailscale
export PATH=$clips/bin:$kit/bin:$HOME/python/bin:$app/Contents/MacOS:/usr/bin:/bin:/usr/sbin:/sbin
export SHELL=/bin/zsh ZDOTDIR=$d/zsh # the panes' zsh reads this prompt, not ~/.zshrc
WINDOW='{"x": 160, "y": 65, "w": 900, "h": 680}' # demo-clips.sh crops to it (2x: pixels); clear of macOS's banners, top right
app_pid() { pgrep -fx "$app/Contents/MacOS/leon" || true; }
# k ARGS: keypost ARGS, only while this app is in front (a dialog that comes up takes nothing)
k() {
  local front
  front=$(lsappinfo info -only pid "$(lsappinfo front)" | tr -dc 0-9)
  [ "$front" = "$(app_pid)" ] || { echo "guest: leon isn't in front (pid ${front:-none}); nothing sent" >&2; exit 1; }
  keypost "$@"
}
pause() { sleep "$1"; }
quit() {
  local p; p=$(app_pid)
  if [ -n "$p" ]; then
    kill $p
    for _ in $(seq 50); do kill -0 $p 2>/dev/null || break; sleep 0.1; done
    kill -9 $p 2>/dev/null || true
  fi
  leon --autostart=false server stop --discard >/dev/null 2>&1 || true
}

stage() {
  quit
  rm -rf "${d:?}" "$HOME/clips-work"
  mkdir -m 700 -p "$d" "$d/run" "$d/state" "$d/zsh"
  printf "PROMPT='\$ '\nunsetopt PROMPT_SP\n" >"$d/zsh/.zshrc"
  mkdir -p "$HOME/clips-work/shelf/src"
  : >"$HOME/clips-work/shelf/src/main.rs"
  git -C "$HOME/clips-work/shelf" init -q --template= -b main
  cat >"$d/init.lua" <<LUA
leon.set("theme", "leon-dark")
leon.set("font.size", 16)
leon.set("notifications.done", false)
leon.set("notifications.blocked", false)
leon.set("notifications.error", false)
leon.set("check_for_updates", false)
LUA
  [ "$1" = cursor-shader ] && printf 'leon.set("cursor_shader", "%s")\nleon.set("cursor_shader_duration", 0.6)\n' "$clips/trail.glsl" >>"$d/init.lua"
  leon server start --restore=false >/dev/null
  local w=$HOME/clips-work/shelf
  case "$1" in
    agents)
      leon new release >/dev/null
      leon -s release run --keep-open -c "$w" -- lead "$d/go" >/dev/null
      leon -s release window close 1 >/dev/null ;;
    link-card)
      leon new docs >/dev/null
      b=$(leon -s docs run --keep-open -c "$w" -- links)
      leon -s docs split -b "$b" --split below --ratio 25 --focus=false -c "$w" -- ci >/dev/null
      leon -s docs window close 1 >/dev/null ;;
    tabs)
      leon new api >/dev/null
      leon new web >/dev/null
      for t in server tests logs; do leon -s api run --keep-open -c "$w" -- zsh -c "printf '\033]2;$t\007'; exec zsh -i" >/dev/null; done
      leon -s api window close 1 >/dev/null ;;
    cursor-shader)
      leon new shelf >/dev/null
      leon -s shelf run --keep-open -c "$w" -- zsh -i >/dev/null
      leon -s shelf window close 1 >/dev/null ;;
  esac
  python3 -c '
import json, sys
json.dump({"tab_style": "sidebar", "pane_headers": True, "sidebar_width": 220,
           "window": json.loads(sys.argv[2]), "display": 1}, open(sys.argv[1], "w"))' "$d/state/app.json" "$WINDOW"
  open --env LEON_RUNTIME_DIR="$LEON_RUNTIME_DIR" --env LEON_STATE_DIR="$LEON_STATE_DIR" \
    --env LEON_CONFIG="$LEON_CONFIG" --env LEON_TAILSCALE="$LEON_TAILSCALE" --env PATH="$PATH" \
    --env SHELL=/bin/zsh --env ZDOTDIR="$ZDOTDIR" "$app"
  for _ in $(seq 50); do [ -n "$(app_pid)" ] && break; sleep 0.1; done
  [ -n "$(app_pid)" ] || { echo "guest: the app didn't start" >&2; exit 1; }
  sleep 3
  k park
}

play() {
  case "$1" in
    agents)
      touch "$d/go"
      pause 14 ;; # it starts both, they work and report, and leon types each notice into its prompt
    link-card)
      pause 1
      k move 630 176 # resting on the link: its card
      pause 3
      k rclick 630 176 # and the right-click card, with Copy and Open in Browser
      pause 3
      k key esc
      k park
      pause 1 ;;
    tabs)
      pause 0.6
      k combo cmd+shift+p; pause 0.5
      k type "rename tab"; pause 0.5; k key return; pause 0.6
      k type "deploy"; pause 0.5; k key return; pause 1.2
      k combo cmd+alt+w; pause 1.4 # Close Tab
      k combo cmd+shift+p; pause 0.5
      k type "rename session"; pause 0.5; k key return; pause 0.6
      k type "shelf api"; pause 0.5; k key return; pause 1.2
      k combo cmd+k; pause 0.5
      k type "web"; pause 0.4; k key return; pause 1
      k combo cmd+alt+shift+w; pause 1.6 ;; # Close Session
    cursor-shader)
      pause 0.6
      k type "git status --short"; pause 0.6
      k combo ctrl+a; pause 0.7 # to the line's start: the trail streaks back
      k combo ctrl+e; pause 0.7 # and to its end
      k key return; pause 1
      k type "ls -1"; pause 0.3; k key return; pause 1.2
      k type "echo the cursor leaves a trail"; pause 0.4
      k combo ctrl+a; pause 0.7
      k combo ctrl+e; pause 1 ;;
  esac
}

case "${1:-}" in
  stage) stage "$2" ;;
  play) play "$2" ;;
  stop) quit ;;
  *) sed -n '2,8s/^# \{0,1\}//p' "$0"; exit 2 ;;
esac
